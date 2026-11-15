# Arch Linux / AUR

`vox-ripple` builds the tagged release from source. The executable and desktop
entry are called `ripple`. The recipe pins its source archive with SHA-256 and
builds the committed Cargo.lock using Rust 1.95 or newer.

## Build locally on Arch

Install base-devel, then run makepkg as a regular user:

```sh
sudo pacman -Syu --needed base-devel git
cd packaging/arch/vox-ripple
makepkg -si
```

The Arch package workflow builds the same recipe in an Arch Linux container and
checks that `.SRCINFO` matches `makepkg --printsrcinfo`. This checks package
creation; it does not exercise a GPU or graphical desktop on Arch.

## Publish to AUR

An AUR account with an authorized SSH public key is required. Check whether the
name is already registered before the first submission. Only the packaging files
belong in the AUR Git repository, not the app source or compiled binaries.

```sh
git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/vox-ripple.git
cd vox-ripple
# Copy PKGBUILD and .SRCINFO from this directory into the cloned repository.
git add PKGBUILD .SRCINFO
git commit -m 'Initial Ripple package'
git push origin master
```

After publication, users can run `yay -S vox-ripple` or `paru -S vox-ripple`.
AUR publication alone does not enable `pacman -S vox-ripple`.

For an update, change pkgver and the source checksum, reset pkgrel to 1, rebuild,
regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO`, and push both files.
Increment pkgrel for packaging changes to the same upstream version.
