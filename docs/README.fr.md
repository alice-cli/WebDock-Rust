# WebRust

Hôte **Rust** pour contrôler une fenêtre depuis le navigateur.  
Réimplémentation multiplateforme de [WebDock](https://github.com/alice-cli/WebDock) (Swift).

**Langues :** [English](../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [中文](README.zh.md) · [Deutsch](README.de.md) · [Français](README.fr.md)

Interface web : EN / KO / JA / ZH / DE / FR.

| Produit | Stack | Bundle ID | Port |
|---------|-------|-----------|------|
| WebDock | Swift | `com.poc.webdock` | 8080 |
| **WebRust** | Rust | `com.poc.webrust` | 8090 |

---

## Fonctions

- Streaming fenêtre / écran (xcap)
- Souris, clavier, défilement, hangeul
- JPEG / PNG / H.264 (macOS : VideoToolbox)
- Jeton d’accès optionnel, LAN

**Sécurité :** jeton fort si le LAN est ouvert.  
**H.264 sur LAN :** WebCodecs exige HTTPS ou localhost.

---

## Installation

```bash
git clone https://github.com/alice-cli/WebDock-Rust.git
cd WebDock-Rust
./setup_dev_cert.sh   # macOS : signature locale (même schéma que WebDock)
./install_home.sh
```

Permissions : **Enregistrement d’écran** + **Accessibilité** → WebRust.

---

## Licence

[MIT](../LICENSE)
