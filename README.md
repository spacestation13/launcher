> [!IMPORTANT]  
> Install directly from GitHub Actions here:
>
> <a href="https://github.com/cmss13-devs/launcher/releases/tag/v0.19.6">
>  <img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows download link"/>
> </a>

# SS13 Launcher ![Steam Build](https://img.shields.io/github/actions/workflow/status/cmss13-devs/cm-launcher/steam.yml?style=for-the-badge&label=STEAM%20BUILD) ![GitHub Build](https://img.shields.io/github/actions/workflow/status/cmss13-devs/cm-launcher/build.yml?style=for-the-badge&label=GITHUB%20BUILD) ![Tests](https://img.shields.io/github/actions/workflow/status/cmss13-devs/cm-launcher/build.yml?style=for-the-badge&label=TESTS)

A launcher for Space Station 13 servers, using [Tauri](https://v2.tauri.app/) and managing BYOND versions internally.

## Screenshots

| CM-SS13 Game Servers                                                                                                                               | Authentication Options (Steam only available in Steam builds)                                                                                      | Automatic Relay Selection                                                                                                                          |
| -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| <img width="1992" height="1188" alt="VcnBDvrlqS7Tfryu@2x" src="https://github.com/user-attachments/assets/d8b5ac37-e818-45cb-b020-5fd96dc64f50" /> | <img width="1981" height="1179" alt="0SR6wKmNaPuefRBK@2x" src="https://github.com/user-attachments/assets/e196bac1-f134-42da-9990-4e4864c24129" /> | <img width="1996" height="1200" alt="6whuDKXeRfZD5E3f@2x" src="https://github.com/user-attachments/assets/b4f08132-6740-4b50-bb91-8f527e2aab5f" /> |

## Features

### BYOND

- Automatically installs the correct version for the game server you are connecting to.
- Private WebView2 install location to avoid conflicts with system BYOND.

### Authentication

- CM-SS13 Authentication via web browser authentication flow
  - Handles tokens refresh to stay logged in indefinitely
- BYOND Authentication via pager
- Steam Authentication via Authentication ticket flow/Authentik backend

### Rich Presence

- Supports Steam and Discord rich presence
- Displays currently launched server, as well as the number of players online
- Allows friends to join directly from the friends list

### CI/CD

- Automatically deploys tagged versions to GitHub Releases and Steam
- Steam releases are pushed to a `latest` branch for manual deployment to `default`.

## Development

Run the project with:

```bash
# both backend and frontend dev (with hotreloading)
npm run tauri -- dev (-f steam) # to build with steam

# production build (cargo will recompile in release)
npm run tauri build
```

In order to run the Steam build in development, you will need to place a file named `steam_appid.txt` in src-tauri/ containing `4313790`. Otherwise, the app will immediately close and attempt to reopen via Steam.

### Releasing

Use `tools/release.sh [semver]` to change the version in `Cargo.toml`, create a commit changing the version, and tag that commit with the semver. When this is pushed, GitHub Actions will push new builds to both GitHub Releases and Steam.

Manually download the `.msi` and `.exe` and upload these to [Microsoft](https://www.microsoft.com/en-us/wdsi/filesubmission) to /try/ and avoid SmartScreen when installed via GitHub Releases.

### To-Do

See issues tagged with https://github.com/cmss13-devs/cm-launcher/labels/feature-request or https://github.com/cmss13-devs/cm-launcher/labels/bug as an easy place to start contributing.
