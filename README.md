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

After deployment, use the CDK `ApiUrl` output as the client API base:

- `VITE_API_BASE_URL`: used by the React Passkey challenge/finish calls.
- `ENERGY_API_BASE_URL`: used by the Tauri Rust API client for snapshot and diary writes.

The current development fallback for both clients is
`https://3ihs6eswbb.execute-api.us-east-1.amazonaws.com`. Environment variables
still override it when targeting another deployment.

For the unsigned iOS GitHub Actions build, set the repository Actions variable
`API_BASE_URL` to that HTTPS `ApiUrl`. A manual workflow run can instead provide
the `api_base_url` input. The workflow injects the value into both client settings
and fails before building when it is missing.

Use only the API base endpoint, such as
`https://example.execute-api.us-east-1.amazonaws.com`. Do not append the API
Gateway route placeholder `/{proxy+}`.

Deploy the iOS API configuration with:

```powershell
npm run infra:deploy:ios
```

This command requires `cargo-lambda` and valid AWS credentials. Install and
authenticate once before deploying:

```powershell
cargo install cargo-lambda
aws configure
```

The iOS app now performs Passkey authentication in Safari at `/auth/app`.
Safari returns to the app through `energylossplus://auth/callback` with a
five-minute, single-use authorization code. The app exchanges that code for a
session token; the token is never placed in the callback URL.

The external Safari page uses the API Gateway HTTPS host as its WebAuthn RP.
After deploying this change, Passkeys created for the old `localhost` RP cannot
be reused and must be registered again.

## Authentication boundary

The app only exposes Passkey/WebAuthn registration and login flows. No password, email-code, SMS, or OAuth fallback is implemented. The Lambda service uses Rust-side WebAuthn ceremony handling, stores registered passkey credentials in DynamoDB, and issues short-lived bearer sessions after successful assertion verification.
