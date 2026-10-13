# pgAdmin Manager

Aplikasi desktop (Rust + GPUI) untuk mengelola pgAdmin 4 yang berjalan sebagai
container **podman/docker**. Data persist di named volume.

## Jalankan

```sh
make run
```

Itu saja — setara `cargo run`.

## Target Makefile lain

| Target            | Fungsi                                            |
|-------------------|---------------------------------------------------|
| `make build`      | build release (`./target/release/pgadmin-manager`)|
| `make check`      | `cargo check` cepat                               |
| `make pull-image` | tarik image pgAdmin                               |
| `make volume-up`  | **presetup named volume** (`pgadmin-data`) agar data persist |
| `make volume-ls`  | lihat + inspect volume                             |
| `make clean`      | hapus `target/`                                   |

Override bila perlu:

```sh
make run RUNTIME=podman HOST_PORT=5050
```

## Fitur aplikasi

- **Start / Stop / Restart / Hapus** container pgAdmin
- Badge status (BERJALAN / BERHENTI / BELUM DIBUAT)
- **Buka Browser** ke `http://localhost:5050` (otomatis setelah Start bila auto-open ON)
- Panel **Konfigurasi**: host port (+/−), email, password, nama container,
  image (+ preset tag `latest`/`9`/`8`), runtime (`auto` → `docker` → `podman`),
  auto-open browser. Semua otomatis tersimpan di
  `~/.config/pgadmin-manager/config.toml`.
- Panel **Volume**: status named volume, tombol *Siapkan Volume* (idempoten)
  dan *Hapus Volume* (konfirmasi 2x — menghapus volume = menghapus semua data).
- Panel **Log**: 30 baris terakhir `logs` container.

## Bagaimana persistensi bekerja

Saat Start (atau via `make volume-up`), aplikasi menjalankan:

```sh
<runtime> volume create pgadmin-data
```

lalu container dibuat dengan:

```sh
<runtime> run -d --name pgadmin4 -p 5050:80 \
  -e PGADMIN_DEFAULT_EMAIL=... -e PGADMIN_DEFAULT_PASSWORD=... \
  -v pgadmin-data:/var/lib/pgadmin \
  --restart unless-stopped \
  docker.io/dpage/pgadmin4:latest
```

Tombol **Hapus** container (`rm -f`) tidak menyentuh volume, jadi data aman.
Hapus data hanya lewat **Hapus Volume**.

## Struktur kode

- `src/main.rs` — UI GPUI (`RootView`: status, aksi, config, volume, log)
- `src/container.rs` — wrapper CLI `docker`/`podman`, deteksi runtime otomatis
- `src/config.rs` — config TOML persisten
- `vendor/xattr` — patch lokal (lihat bawah)

## Catatan build

`gpui 0.2.2` menarik `zed-async-tar` → `xattr 0.2.3`, yang gagal dikompilasi
dengan `libc` baru (`ENOATTR` dihapus). `vendor/xattr` adalah salinan 0.2.3
dengan fallback ke `ENODATA` di Linux, dipasang via `[patch.crates-io]`
di `Cargo.toml`. Kalau suatu hari dependensi hulu diperbaiki, hapus saja
folder `vendor/` + blok patch tersebut.

## Kebutuhan sistem

- Rust stabil terbaru, `podman` dan/atau `docker`
- Linux: Wayland/X11 headers (sudah ada bila pakai desktop Arch standar)
