# Workbox (vendored)

Berkas di folder ini adalah runtime resmi [Workbox](https://github.com/GoogleChrome/workbox)
versi **7.4.1**, diambil apa adanya dari paket npm `workbox-sw` dan modul
`workbox-core`, `workbox-routing`, `workbox-strategies`, `workbox-precaching`,
`workbox-expiration`, serta `workbox-cacheable-response` (folder `build/`).

| Berkas | Paket asal |
| --- | --- |
| `workbox-sw.js` | `workbox-sw` |
| `workbox-core.prod.js` | `workbox-core` |
| `workbox-routing.prod.js` | `workbox-routing` |
| `workbox-strategies.prod.js` | `workbox-strategies` |
| `workbox-precaching.prod.js` | `workbox-precaching` |
| `workbox-expiration.prod.js` | `workbox-expiration` |
| `workbox-cacheable-response.prod.js` | `workbox-cacheable-response` |

Alasannya di-vendor, bukan diambil dari CDN: service worker harus bisa
terpasang saat perangkat offline, dan aplikasi tidak boleh bergantung pada
ketersediaan domain pihak ketiga saat runtime.

`selarust-build` menyalin berkas-berkas ini ke `<assets_dir>/workbox/` pada setiap
build, lalu `sw.js` yang digenerate memuatnya lewat
`importScripts("<prefix>workbox/workbox-sw.js")` dengan `modulePathPrefix` yang
sama. Untuk memperbarui: `npm i workbox-sw@7 workbox-core@7 workbox-routing@7
workbox-strategies@7 workbox-precaching@7 workbox-expiration@7
workbox-cacheable-response@7`, lalu salin ulang `build/*.prod.js` dan versi di
tabel ini.

Lisensi: MIT — Copyright 2018 Google LLC, salinan teksnya ada di
[`LICENSE.workbox`](./LICENSE.workbox).
