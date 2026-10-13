//! Penyimpanan todo sisi klien: snapshot lokal, antrean mutasi offline, dan
//! penerapan event sinkronisasi.
//!
//! Modelnya sengaja sederhana dan bisa diuji tanpa browser:
//!
//! * `server` — daftar terakhir dari server (SSR/refetch).
//! * `snapshot` — salinan di IndexedDB, dipakai saat server tidak terjangkau.
//! * `pending` — mutasi yang belum tersinkron; diterapkan sebagai *overlay*
//!   optimistis di atas daftar dasar.
//!
//! Semua fungsi murni (`coalesce`, `translate`, `overlay`) diuji unit; bagian
//! yang menyentuh browser (IndexedDB, server functions) hanya membungkusnya.

use std::collections::HashMap;

use leptos::prelude::*;
use sayagi::{is_online, offline};
use serde::{Deserialize, Serialize};

use crate::api::{
    create_todo, delete_todo, list_todos, set_todo_completed, update_todo, Todo, TodoEvent,
};

/// Kunci snapshot daftar todo di IndexedDB.
const SNAPSHOT_KEY: &str = "todos";

/// Batas putaran replay: mencegah loop panjang bila mutasi terus bertambah.
const FLUSH_ROUNDS: usize = 4;

/// Mutasi lokal yang belum sampai ke server.
///
/// `temp_id` (negatif) dipakai baris yang dibuat saat offline; saat replay, id
/// asli dari server dipetakan lewat [`translate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TodoMutation {
    /// Todo baru yang belum punya id server.
    Create {
        /// Id sementara (selalu negatif).
        temp_id: i64,
        /// Judul.
        title: String,
        /// Catatan.
        notes: String,
    },
    /// Perubahan judul/catatan.
    Update {
        /// Id baris (bisa id sementara).
        id: i64,
        /// Judul baru.
        title: String,
        /// Catatan baru.
        notes: String,
    },
    /// Perubahan status selesai.
    Complete {
        /// Id baris.
        id: i64,
        /// Nilai baru.
        completed: bool,
    },
    /// Penghapusan.
    Delete {
        /// Id baris.
        id: i64,
    },
}

/// Membuang mutasi yang hasilnya sudah tercakup mutasi berikutnya.
///
/// Hanya berlaku untuk baris yang sudah punya id: `Create` tidak bisa digabung
/// karena id server-nya baru diketahui saat replay.
pub fn coalesce(mutations: Vec<TodoMutation>) -> Vec<TodoMutation> {
    let mut out: Vec<TodoMutation> = Vec::with_capacity(mutations.len());
    for mutation in mutations {
        match (&mutation, out.last()) {
            // Dua `Update` berurutan untuk baris yang sama: yang terakhir menang.
            (TodoMutation::Update { id, .. }, Some(TodoMutation::Update { id: previous, .. }))
                if id == previous =>
            {
                out.pop();
            }
            // Dua `Complete` berurutan: yang terakhir menang.
            (
                TodoMutation::Complete { id, .. },
                Some(TodoMutation::Complete { id: previous, .. }),
            ) if id == previous => {
                out.pop();
            }
            // Baris yang dihapus tidak perlu diubah lagi.
            (
                TodoMutation::Delete { id },
                Some(TodoMutation::Update { id: previous, .. } | TodoMutation::Complete { id: previous, .. }),
            ) if id == previous => {
                out.pop();
            }
            _ => {}
        }
        out.push(mutation);
    }
    out
}

/// Mengganti id sementara dengan id server hasil replay `Create`.
pub fn translate(mutation: &TodoMutation, remap: &HashMap<i64, i64>) -> TodoMutation {
    let resolve = |id: i64| remap.get(&id).copied().unwrap_or(id);
    match mutation {
        TodoMutation::Create { .. } => mutation.clone(),
        TodoMutation::Update { id, title, notes } => TodoMutation::Update {
            id: resolve(*id),
            title: title.clone(),
            notes: notes.clone(),
        },
        TodoMutation::Complete { id, completed } => TodoMutation::Complete {
            id: resolve(*id),
            completed: *completed,
        },
        TodoMutation::Delete { id } => TodoMutation::Delete { id: resolve(*id) },
    }
}

/// Menerapkan mutasi yang belum tersinkron ke atas daftar dasar.
pub fn overlay(base: &[Todo], pending: &[TodoMutation]) -> Vec<Todo> {
    let mut items: Vec<Todo> = base.to_vec();
    // Baris baru dikumpulkan terpisah lalu ditaruh di depan: server mengurutkan
    // todo terbaru lebih dulu, jadi baris optimistis harus muncul di posisi yang
    // sama seperti nanti setelah replay.
    let mut created: Vec<Todo> = Vec::new();
    for mutation in pending {
        match mutation {
            TodoMutation::Create {
                temp_id,
                title,
                notes,
            } => {
                created.insert(
                    0,
                    Todo {
                        id: *temp_id,
                        title: title.clone(),
                        notes: notes.clone(),
                        completed: false,
                        created_at: String::new(),
                    },
                );
            }
            TodoMutation::Update { id, title, notes } => {
                if let Some(todo) = items.iter_mut().find(|todo| todo.id == *id) {
                    todo.title = title.clone();
                    todo.notes = notes.clone();
                }
            }
            TodoMutation::Complete { id, completed } => {
                if let Some(todo) = items.iter_mut().find(|todo| todo.id == *id) {
                    todo.completed = *completed;
                }
            }
            TodoMutation::Delete { id } => items.retain(|todo| todo.id != *id),
        }
    }
    created.append(&mut items);
    created
}

/// State klien untuk daftar todo, dibagikan lewat context dari akar aplikasi.
#[derive(Clone, Copy)]
pub struct TodoStore {
    /// Daftar terakhir dari server.
    server: RwSignal<Option<Vec<Todo>>>,
    /// Salinan lokal (IndexedDB) yang bertahan antar sesi.
    snapshot: RwSignal<Vec<Todo>>,
    /// Mutasi yang menunggu sinkronisasi.
    pending: RwSignal<Vec<TodoMutation>>,
    /// True begitu ada sumber data di klien (snapshot lokal atau server).
    ///
    /// Selama `false` (termasuk saat SSR) halaman merender data dari `Resource`,
    /// sehingga HTML hasil SSR tetap berisi daftar; setelah hidrasi store yang
    /// mengambil alih.
    ready: RwSignal<bool>,
    /// Penanda sinkronisasi sedang berjalan.
    syncing: RwSignal<bool>,
    /// Galat terakhir saat replay, untuk ditampilkan ke pengguna.
    error: RwSignal<Option<String>>,
    /// Counter id sementara (selalu negatif).
    next_temp: RwSignal<i64>,
}

impl TodoStore {
    /// Membuat store kosong; pemuatan snapshot dilakukan oleh [`TodoStore::load`].
    pub fn new() -> Self {
        Self {
            server: RwSignal::new(None),
            snapshot: RwSignal::new(Vec::new()),
            ready: RwSignal::new(false),
            pending: RwSignal::new(Vec::new()),
            syncing: RwSignal::new(false),
            error: RwSignal::new(None),
            next_temp: RwSignal::new(-1),
        }
    }

    /// Daftar yang ditampilkan: server (atau snapshot) + overlay mutasi pending.
    pub fn items(self) -> Signal<Vec<Todo>> {
        Signal::derive(move || {
            let base = self
                .server
                .get()
                .unwrap_or_else(|| self.snapshot.get());
            overlay(&base, &self.pending.get())
        })
    }

    /// Apakah store sudah punya data klien (snapshot atau server).
    pub fn ready(&self) -> Signal<bool> {
        self.ready.into()
    }

    /// Jumlah mutasi yang belum tersinkron.
    pub fn pending_count(&self) -> Signal<usize> {
        let pending = self.pending;
        Signal::derive(move || pending.with(Vec::len))
    }

    /// Apakah replai sedang berjalan.
    pub fn syncing(&self) -> Signal<bool> {
        self.syncing.into()
    }

    /// Galat replay terakhir (mis. server menolak satu mutasi).
    pub fn error(&self) -> Signal<Option<String>> {
        self.error.into()
    }

    /// Membaca snapshot dari IndexedDB (sekali, saat aplikasi dimulai).
    pub async fn load(&self) {
        if let Some(raw) = offline::snapshot(SNAPSHOT_KEY).await {
            if let Ok(items) = serde_json::from_str::<Vec<Todo>>(&raw) {
                self.snapshot.set(items);
            }
        }
        if let Ok(queued) = offline::queue_all::<TodoMutation>().await {
            self.pending
                .set(coalesce(queued.into_iter().map(|entry| entry.payload).collect()));
        }
        self.ready.set(true);
    }

    /// Menyimpan daftar server sebagai snapshot lokal.
    pub fn remember(&self, items: Vec<Todo>) {
        self.server.set(Some(items.clone()));
        self.snapshot.set(items.clone());
        self.ready.set(true);
        let encoded = match serde_json::to_string(&items) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.error.set(Some(format!("snapshot: {error}")));
                return;
            }
        };
        leptos::task::spawn_local(async move {
            let _ = offline::store_snapshot(SNAPSHOT_KEY, &encoded).await;
        });
    }

    /// Mengganti daftar server (dipakai olelh refetch dan snapshot WebSocket).
    pub fn replace(&self, items: Vec<Todo>) {
        self.remember(items);
    }

    /// Menerapkan satu event sinkronisasi ke daftar lokal.
    pub fn apply(&self, event: &TodoEvent) {
        let mut items = self
            .server
            .get()
            .unwrap_or_else(|| self.snapshot.get());
        event.apply(&mut items);

        // Baris sementara yang sudah punya id asli dibuang supaya tidak ganda.
        if let TodoEvent::Created { todo } = event {
            items.retain(|existing| existing.id >= 0 || existing.id == todo.id);
        }
        self.remember(items);
    }

    /// Menambahkan mutasi ke antrean dan menerapkannya secara optimistis.
    pub async fn enqueue(&self, mutation: TodoMutation) {
        match offline::queue_push(&mutation).await {
            Ok(_) => {
                self.pending.update(|pending| {
                    pending.push(mutation.clone());
                    *pending = coalesce(pending.clone());
                });
            }
            Err(error) => self.error.set(Some(error)),
        }
    }

    /// Id sementara berikutnya untuk baris baru saat offline.
    pub fn next_temp_id(&self) -> i64 {
        let id = self.next_temp.get_untracked();
        self.next_temp.set(id - 1);
        id
    }

    /// Memutar ulang antrean ke server, berurutan.
    ///
    /// Berhenti pada kegagalan pertama supaya urutan mutasi tidak rusak; sisa
    /// antrean dicoba lagi saat koneksi kembali. Putaran diulang (maksimal
    /// [`FLUSH_ROUNDS`]) supaya mutasi yang masuk *selama* replay ikut terkirim.
    pub async fn flush(&self) {
        if self.syncing.get_untracked() {
            return;
        }
        if self.pending.get_untracked().is_empty() {
            return;
        }
        if !is_online() {
            return;
        }

        self.syncing.set(true);
        self.error.set(None);

        let mut replayed = 0usize;
        for _ in 0..FLUSH_ROUNDS {
            let queued = offline::queue_all::<TodoMutation>().await.unwrap_or_default();
            if queued.is_empty() {
                break;
            }
            let sent = self.replay_all(queued).await;
            replayed += sent;
            if sent == 0 {
                break;
            }
        }

        // Antrean yang tersisa dibaca ulang dari IndexedDB: sumber kebenarannya
        // adalah penyimpanan, bukan salinan di memori.
        let remaining = offline::queue_all::<TodoMutation>().await.unwrap_or_default();
        self.pending.set(coalesce(
            remaining.into_iter().map(|entry| entry.payload).collect(),
        ));
        self.syncing.set(false);

        if replayed > 0 {
            self.refresh().await;
        }
    }

    /// Replays one batch, stopping at the first failure. Returns how many landed.
    async fn replay_all(&self, queued: Vec<sayagi::QueuedMutation<TodoMutation>>) -> usize {
        let mut remap: HashMap<i64, i64> = HashMap::new();
        let mut replayed = 0usize;

        for entry in queued {
            let mutation = translate(&entry.payload, &remap);
            match self.replay(&mutation, &mut remap).await {
                Ok(()) => {
                    let _ = offline::queue_remove(entry.key).await;
                    replayed += 1;
                }
                Err(error) => {
                    self.error.set(Some(error));
                    break;
                }
            }
        }
        replayed
    }

    async fn replay(
        &self,
        mutation: &TodoMutation,
        remap: &mut HashMap<i64, i64>,
    ) -> Result<(), String> {
        let text = |error: ServerFnError| error.to_string();
        match mutation {
            TodoMutation::Create {
                temp_id,
                title,
                notes,
            } => {
                let todo = create_todo(title.clone(), Some(notes.clone()))
                    .await
                    .map_err(text)?;
                remap.insert(*temp_id, todo.id);
                self.error.set(None);
                Ok(())
            }
            TodoMutation::Update { id, title, notes } => update_todo(*id, title.clone(), Some(notes.clone()))
                .await
                .map(|_| ())
                .map_err(text),
            TodoMutation::Complete { id, completed } => set_todo_completed(*id, *completed)
                .await
                .map_err(text),
            TodoMutation::Delete { id } => delete_todo(*id).await.map_err(text),
        }
    }

    /// Mengambil daftar segar dari server.
    pub async fn refresh(&self) {
        match list_todos().await {
            Ok(items) => self.remember(items),
            Err(error) => self.error.set(Some(error.to_string())),
        }
    }
}

impl Default for TodoStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{coalesce, overlay, translate, TodoMutation};
    use crate::api::Todo;
    use std::collections::HashMap;

    fn todo(id: i64, title: &str, completed: bool) -> Todo {
        Todo {
            id,
            title: title.to_string(),
            notes: String::new(),
            completed,
            created_at: "2026-01-01 00:00:00".to_string(),
        }
    }

    #[test]
    fn consecutive_updates_and_completions_collapse() {
        let mutations = vec![
            TodoMutation::Update {
                id: 7,
                title: "pertama".into(),
                notes: String::new(),
            },
            TodoMutation::Update {
                id: 7,
                title: "kedua".into(),
                notes: String::new(),
            },
            TodoMutation::Complete {
                id: 7,
                completed: true,
            },
            TodoMutation::Complete {
                id: 7,
                completed: false,
            },
        ];

        assert_eq!(
            coalesce(mutations),
            vec![
                TodoMutation::Update {
                    id: 7,
                    title: "kedua".into(),
                    notes: String::new(),
                },
                TodoMutation::Complete {
                    id: 7,
                    completed: false,
                },
            ]
        );
    }

    #[test]
    fn delete_wins_over_pending_edits_of_the_same_row() {
        let mutations = vec![
            TodoMutation::Complete {
                id: 3,
                completed: true,
            },
            TodoMutation::Delete { id: 3 },
        ];

        assert_eq!(coalesce(mutations), vec![TodoMutation::Delete { id: 3 }]);
    }

    #[test]
    fn coalesce_keeps_creates_and_unrelated_rows() {
        let mutations = vec![
            TodoMutation::Create {
                temp_id: -1,
                title: "baru".into(),
                notes: String::new(),
            },
            TodoMutation::Complete {
                id: 1,
                completed: true,
            },
            TodoMutation::Complete {
                id: 2,
                completed: true,
            },
        ];

        assert_eq!(coalesce(mutations.clone()), mutations);
    }

    #[test]
    fn translate_rewrites_temporary_ids_after_replay() {
        let mut remap = HashMap::new();
        remap.insert(-1, 42);

        assert_eq!(
            translate(
                &TodoMutation::Complete {
                    id: -1,
                    completed: true
                },
                &remap
            ),
            TodoMutation::Complete {
                id: 42,
                completed: true
            }
        );
        assert_eq!(
            translate(&TodoMutation::Delete { id: -9 }, &remap),
            TodoMutation::Delete { id: -9 }
        );
    }

    #[test]
    fn overlay_projects_pending_mutations_onto_the_base_list() {
        let base = vec![todo(2, "lama", false), todo(1, "hapus aku", false)];
        let pending = vec![
            TodoMutation::Complete {
                id: 2,
                completed: true,
            },
            TodoMutation::Delete { id: 1 },
            TodoMutation::Create {
                temp_id: -1,
                title: "offline".into(),
                notes: "catatan".into(),
            },
        ];

        let items = overlay(&base, &pending);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, -1);
        assert_eq!(items[0].title, "offline");
        assert!(items[1].completed);
        assert!(items.iter().all(|todo| todo.id != 1));
    }
}
