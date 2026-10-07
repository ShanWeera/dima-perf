# DiMA application icon

`dima.svg` is the source: an entropy trace over a baseline with one diverse peak, in Okabe–Ito
blue and vermillion (Wong 2011, doi:10.1038/nmeth.1618). It is used for the desktop app's window,
the Windows executable and installer, the macOS bundle and the Linux packages (plan D80).

The raster files are generated, never edited by hand:

```bash
python3 assets/icon/generate.py
```

The script uses the Python standard library only and is deterministic, so the outputs are
byte-for-byte reproducible. It draws the same geometry as `dima.svg` (keep the two in step) with
8×8 supersampled anti-aliasing and writes:

| File | Used by |
|---|---|
| `dima-{16,32,48,64,128,256,512,1024}.png` | window icon (`dima-256.png`, embedded), Linux `.deb`/AppImage, docs |
| `dima.ico` (16–256 px, PNG entries) | Windows executable (embedded by `crates/dima-gui/build.rs`) and `.msi` |
| `dima.icns` (16–1024 px, PNG entries) | macOS `.app`/`.dmg` |

Packaging metadata (cargo-packager) is in `crates/dima-gui/Cargo.toml`.
