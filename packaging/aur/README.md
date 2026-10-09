# AUR package

`PKGBUILD` and `.SRCINFO` for the [`termtype`](https://aur.archlinux.org/packages/termtype)
AUR package, which builds a tagged release from source.

Build and install it locally:

```sh
makepkg -si
```

## Publishing a new version

1. Tag and push the release (`git tag -a vX.Y.Z && git push origin vX.Y.Z`).
2. Here, set `pkgver` to the new version and `pkgrel=1`, then update the
   checksum and `.SRCINFO`:

   ```sh
   updpkgsums            # from pacman-contrib
   makepkg -f            # builds and runs the tests
   makepkg --printsrcinfo > .SRCINFO
   ```

3. Copy `PKGBUILD` and `.SRCINFO` into a clone of
   `ssh://aur@aur.archlinux.org/termtype.git`, commit and push.
