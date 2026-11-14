#!/usr/bin/env bash
# Packages a release build of Blockwise for one system, ready to download.
# The release workflow (.github/workflows/release.yml) runs it; you can too.
#
#   ./package.sh macos    → dist/Blockwise-<version>-macOS.dmg
#                           (one app for Apple silicon and Intel)
#   ./package.sh linux    → dist/Blockwise-<version>-Linux-<arch>.tar.gz
#   ./package.sh windows  → dist/Blockwise-<version>-Windows-<arch>.zip
#
# Every package carries LICENSE, NOTICE, THIRD_PARTY.md and licenses/, which
# the Apache License and the bundled assets' licenses ask for.
set -euo pipefail
cd "$(dirname "$0")"

system=${1:?usage: ./package.sh macos|linux|windows}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
root=..
stage=target/package
out=dist
rm -rf "$stage"
mkdir -p "$stage" "$out"

legal() { # copy the license files into $1
  mkdir -p "$1"
  cp "$root/LICENSE" "$root/NOTICE" "$root/THIRD_PARTY.md" "$1/"
  cp -R "$root/licenses" "$1/licenses"
}

arch() {
  case "$(uname -m)" in
    x86_64 | amd64) echo x86_64 ;;
    aarch64 | arm64) echo arm64 ;;
    *) uname -m ;;
  esac
}

case "$system" in
  macos)
    # Apple silicon runs from 11.0; keep Intel at the same floor
    export MACOSX_DEPLOYMENT_TARGET=11.0
    targets=(aarch64-apple-darwin x86_64-apple-darwin)
    for t in "${targets[@]}"; do
      if ! rustup target list --installed | grep -qx "$t"; then
        echo "package.sh: Rust target $t is missing; add it with: rustup target add $t" >&2
        exit 1
      fi
      cargo build --release --locked --target "$t"
    done

    app="$stage/dmg/Blockwise.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    lipo -create -output "$app/Contents/MacOS/Blockwise" \
      target/aarch64-apple-darwin/release/Blockwise target/x86_64-apple-darwin/release/Blockwise
    cp assets/brand/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
    legal "$app/Contents/Resources"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Blockwise</string>
  <key>CFBundleDisplayName</key><string>Blockwise</string>
  <key>CFBundleIdentifier</key><string>dev.eledrissi.blockwise</string>
  <key>CFBundleExecutable</key><string>Blockwise</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Copyright 2026 Aissam Abouqassime El Edrissi. Apache License 2.0.</string>
  <!-- .bw project files open in Blockwise -->
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Blockwise project</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>CFBundleTypeIconFile</key><string>AppIcon</string>
      <key>LSItemContentTypes</key><array><string>dev.eledrissi.blockwise.project</string></array>
    </dict>
  </array>
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>dev.eledrissi.blockwise.project</string>
      <key>UTTypeDescription</key><string>Blockwise project</string>
      <key>UTTypeIconFile</key><string>AppIcon</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string></array>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key><array><string>bw</string></array>
        <key>public.mime-type</key><array><string>application/x-blockwise-project</string></array>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST
    # an ad-hoc signature: Apple silicon needs one to run at all. It is not a
    # Developer ID, so macOS still asks people to allow the app the first time.
    codesign --force --deep --sign - "$app"
    ln -s /Applications "$stage/dmg/Applications"
    file="$out/Blockwise-$version-macOS.dmg"
    rm -f "$file"
    hdiutil create -volname "Blockwise $version" -srcfolder "$stage/dmg" -fs HFS+ -format UDZO -ov "$file" >/dev/null
    ;;

  linux)
    cargo build --release --locked
    name="Blockwise-$version-Linux-$(arch)"
    dir="$stage/$name"
    mkdir -p "$dir"
    cp target/release/Blockwise "$dir/"
    cp assets/brand/icon-512.png "$dir/blockwise.png"
    # named after the app id (dev.eledrissi.blockwise), which the window gives too
    cat > "$dir/dev.eledrissi.blockwise.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Blockwise
Comment=Build Android and iOS apps visually
Exec=Blockwise %f
Icon=blockwise
Categories=Development;
MimeType=application/x-blockwise-project;
StartupWMClass=dev.eledrissi.blockwise
DESKTOP
    legal "$dir"
    file="$out/$name.tar.gz"
    tar -czf "$file" -C "$stage" "$name"
    ;;

  windows)
    cargo build --release --locked
    name="Blockwise-$version-Windows-$(arch)"
    dir="$stage/$name"
    mkdir -p "$dir"
    cp target/release/Blockwise.exe "$dir/"
    legal "$dir"
    file="$out/$name.zip"
    rm -f "$file"
    if command -v 7z >/dev/null; then
      (cd "$stage" && 7z a -tzip -bso0 "../../$file" "$name")
    else
      (cd "$stage" && zip -qr "../../$file" "$name")
    fi
    ;;

  *)
    echo "package.sh: unknown system '$system' (use macos, linux or windows)" >&2
    exit 1
    ;;
esac

echo "$file"
