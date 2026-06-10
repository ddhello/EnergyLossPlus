# EnergyLossPlus

EnergyLossPlus is a cross-platform calorie control desktop app scaffold built with React, Tauri, Rust, AWS Lambda, and DynamoDB.

## What is included

- `apps/desktop`: React UI plus Tauri Rust commands.
- `crates/core`: shared Rust calorie, TDEE, target, and domain models.
- `services/api`: Rust Lambda API with Passkey-only route contracts and DynamoDB storage.
- `infra`: AWS CDK stack for API Gateway, Lambda, and DynamoDB.

The v1 diary supports cloud-confirmed create, edit, and delete operations for food, exercise, and weight records. The desktop client writes through Tauri Rust commands first and updates the local SQLite cache only after the Lambda API confirms the cloud write.

## Local workflow

```powershell
npm install
npm run dev
cargo test --workspace
```

Use `npm run tauri:dev` after Tauri prerequisites are installed.

## AWS deployment workflow

The CDK stack builds the Rust Lambda with Docker using the cargo-lambda image. Docker must be installed and visible on `PATH` before running synth or deploy.

```powershell
npm --workspace infra run build
npm --workspace infra run synth
npm run infra:deploy
```

Configure these environment variables for deployed Passkey origins:

- `WEBAUTHN_RP_ID`: bare relying-party host, for example `app.example.com`.
- `WEBAUTHN_RP_NAME`: display name, defaults to `EnergyLossPlus`.
- `WEBAUTHN_ORIGIN`: browser origin, for example `https://app.example.com`.

For packaged Tauri desktop builds, the window is configured with `useHttpsScheme` so Windows and Android use an HTTPS localhost-style app origin. Keep the Lambda `WEBAUTHN_ORIGIN` aligned with the actual desktop/web origin used for a release.

After deployment, use the CDK `ApiUrl` output as the client API base:

- `VITE_API_BASE_URL`: used by the React Passkey challenge/finish calls.
- `ENERGY_API_BASE_URL`: used by the Tauri Rust API client for snapshot and diary writes.

For the unsigned iOS GitHub Actions build, set the repository Actions variable
`API_BASE_URL` to that HTTPS `ApiUrl`. A manual workflow run can instead provide
the `api_base_url` input. The workflow injects the value into both client settings
and fails before building when it is missing.

Use only the API base endpoint, such as
`https://example.execute-api.us-east-1.amazonaws.com`. Do not append the API
Gateway route placeholder `/{proxy+}`.

Packaged iOS Tauri apps use the `tauri://localhost` origin. Deploy the CDK stack
with `WebauthnOrigin=tauri://localhost`; otherwise API Gateway CORS blocks the
Passkey challenge request and iOS reports `Load failed`.

Deploy the iOS API configuration with:

```powershell
npm run infra:deploy:ios
```

## Authentication boundary

The app only exposes Passkey/WebAuthn registration and login flows. No password, email-code, SMS, or OAuth fallback is implemented. The Lambda service uses Rust-side WebAuthn ceremony handling, stores registered passkey credentials in DynamoDB, and issues short-lived bearer sessions after successful assertion verification.
