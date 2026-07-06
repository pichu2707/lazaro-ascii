# Cómo publicar lazarobox-ascii

Guía paso a paso para publicar la herramienta por primera vez. Hay **tres formas de
instalación** para el usuario final y aquí se preparan todas:

- `cargo install lazarobox-ascii` → **crates.io** (Parte 1)
- `brew install pichu2707/tap/lazarobox-ascii` → **Homebrew** (Partes 2 y 3)
- instalador de una línea (`curl … | sh`) → se genera solo con el release (Parte 3)

> Todo lo del código ya está listo y verificado. Aquí solo van los pasos que necesitan
> **tus cuentas y tokens** (GitHub y crates.io), que nadie puede hacer por ti.

---

## Parte 1 — Publicar en crates.io (para `cargo install`)

Es la más independiente; puedes hacerla cuando quieras.

1. Entra en **https://crates.io** y pulsa **Log in with GitHub** (arriba a la derecha).
2. Ve a **Account Settings** → **API Tokens** → **New Token**.
   - Nombre: `lazarobox-ascii` (o el que quieras).
   - Marca los permisos **publish-new** y **publish-update**.
   - Pulsa **Generate** y **copia el token** (solo se muestra una vez).
3. En tu terminal, dentro del proyecto:
   ```bash
   cargo login <PEGA_AQUÍ_TU_TOKEN>
   cargo publish
   ```
4. Listo. En un par de minutos cualquiera podrá hacer `cargo install lazarobox-ascii`.

> ⚠️ El nombre `lazarobox-ascii` debe estar **libre** en crates.io. Si estuviera cogido,
> `cargo publish` dará error: cambia `name` en `Cargo.toml` por otro disponible.
>
> ⚠️ Una versión publicada **no se puede borrar** (solo "yank" para ocultarla). Publica
> cuando estés seguro.

---

## Parte 2 — Preparar Homebrew (una sola vez)

Para que el release publique la fórmula de Homebrew necesitas un **repo tap** y un **token**.

### 2.1 Crear el repositorio del tap

1. Ve a **https://github.com/new**.
2. Owner: `pichu2707`. Repository name: **`homebrew-tap`** (nombre exacto).
3. Visibilidad: **Public**.
4. No marques nada más (sin README). Pulsa **Create repository**. Déjalo vacío.

### 2.2 Crear el token y guardarlo como secret

1. Ve a **https://github.com/settings/tokens** → **Generate new token (classic)**.
   - Note: `homebrew-tap lazarobox`.
   - Expiration: la que prefieras (p. ej. 90 días o *No expiration*).
   - Scope: marca **`repo`** (acceso completo a repos).
   - Pulsa **Generate token** y **copia el valor**.
2. Ve a tu repo del proyecto: **https://github.com/pichu2707/lazaro-scii/settings/secrets/actions**.
3. Pulsa **New repository secret**:
   - Name: **`HOMEBREW_TAP_TOKEN`** (nombre exacto, respeta mayúsculas).
   - Secret: pega el token.
   - Pulsa **Add secret**.

---

## Parte 3 — Lanzar el release (binarios + instalador + Homebrew)

> Haz esto **después** de la Parte 2, o el paso de Homebrew fallará.

1. En tu terminal, dentro del proyecto:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
2. Ve a **https://github.com/pichu2707/lazaro-scii/actions** y verás el workflow **Release**
   en marcha. Tarda unos minutos (compila para macOS, Linux y Windows).
3. Cuando termine en verde:
   - En **Releases** del repo tendrás los binarios y el instalador.
   - En `pichu2707/homebrew-tap` aparecerá la fórmula `lazarobox-ascii.rb`.

---

## Cómo instalarán tus usuarios

Una vez publicado:

```bash
# Homebrew (macOS / Linux)
brew install pichu2707/tap/lazarobox-ascii

# Rust
cargo install lazarobox-ascii

# Instalador directo (Linux / macOS)
curl -LsSf https://github.com/pichu2707/lazaro-scii/releases/download/v0.1.0/lazarobox-ascii-installer.sh | sh
```

---

## Publicar versiones futuras

1. Sube la versión en `Cargo.toml` (p. ej. `version = "0.2.0"`).
2. `git commit` de los cambios y `git push`.
3. `cargo publish` (para crates.io).
4. `git tag v0.2.0 && git push origin v0.2.0` (para binarios + Homebrew).

`dist` y la fórmula se actualizan solos con cada tag.

---

## Si algo falla

- **`cargo publish` dice que el nombre está cogido** → cambia `name` en `Cargo.toml`.
- **El job de Homebrew falla** → revisa que el repo `homebrew-tap` exista y que el secret
  se llame exactamente `HOMEBREW_TAP_TOKEN`.
- **Quieres probar sin publicar** → `cargo publish --dry-run` (crates.io) y `dist plan`
  (release) muestran qué pasaría sin subir nada.
