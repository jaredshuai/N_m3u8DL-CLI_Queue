# Release guide

This repository has three GitHub Actions workflows:

- `CI`: validates the Tauri/Svelte/Rust GUI on pull requests and pushes.
- `Build_Latest`: keeps the legacy .NET CLI artifact build available with current GitHub Actions versions.
- `Release`: builds the Windows desktop GUI package and publishes GitHub Release assets.

## Normal release flow

1. Prepare and commit the new GUI version:

   ```bash
   cd m3u8-queue-downloader
   npm run release:prepare -- 0.2.0
   cd ..
   git commit -am "chore(release): v0.2.0"
   ```

2. Push the release tag:

   ```bash
   git tag app-v0.2.0
   git push origin master app-v0.2.0
   ```

3. GitHub Actions runs `Release`, builds the legacy CLI, copies it into `src-tauri/resources/N_m3u8DL-CLI_v3.0.2.exe`, then runs the Tauri release build.

4. Check the generated GitHub Release assets before publishing widely. The workflow defaults to a published release for tag pushes and a draft release for manual `workflow_dispatch` runs.

## Manual draft release

Use the GitHub Actions tab, choose `Release`, then run the workflow manually. This is useful for smoke testing installers before creating or pushing a release tag.

## Notes

- Windows packages are produced first because the GUI currently wraps the Windows .NET Framework CLI and includes Windows-specific behavior such as tray behavior and optional shutdown.
- Code signing is not configured. Unsigned installers may trigger Windows SmartScreen warnings until a signing certificate is added.
- The bundled CLI name is intentionally kept as `N_m3u8DL-CLI_v3.0.2.exe` because the GUI launcher searches for that executable name.
