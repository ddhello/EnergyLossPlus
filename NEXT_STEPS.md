# EnergyLossPlus 手动处理指南

这个文件记录接下来需要手动完成的事项。当前项目代码、测试和桌面安装包构建已经通过；主要剩余工作是准备 AWS/Docker 环境、部署后端，并把客户端指向真实 API。

## 1. 本机前置条件

确认这些命令可用：

```powershell
node --version
npm --version
cargo --version
rustc --version
```

当前项目使用的是通过 nvm 安装的 Node.js。若 PowerShell 找不到 `npm`，先把 nvm 的 Node 目录加入当前会话：

```powershell
$env:Path = "$env:LOCALAPPDATA\nvm\v26.3.0;$env:Path"
```

若 PowerShell 找不到 `cargo`，把 Rust 工具链目录加入当前会话：

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

CDK 打包 Rust Lambda 需要 Docker。安装并启动 Docker Desktop 后确认：

```powershell
docker --version
docker info
```

## 2. 获得 AWS 凭证

推荐方式是使用 AWS IAM Identity Center 或 AWS CLI 的 profile，而不是把长期 access key 写进项目文件。

### 方式 A：AWS IAM Identity Center 推荐

1. 安装 AWS CLI v2。
2. 在 AWS 控制台启用或使用已有的 IAM Identity Center。
3. 给你的用户分配目标 AWS 账号和权限集。开发阶段通常需要能创建 CloudFormation、Lambda、API Gateway、DynamoDB、IAM Role、S3/CDK assets 等资源。
4. 在本机配置 SSO：

```powershell
aws configure sso
```

5. 按提示选择 start URL、region、account、role，并保存为一个 profile，例如：

```text
energylossplus-dev
```

6. 登录：

```powershell
aws sso login --profile energylossplus-dev
```

7. 验证凭证：

```powershell
aws sts get-caller-identity --profile energylossplus-dev
```

部署时使用：

```powershell
$env:AWS_PROFILE = "energylossplus-dev"
```

### 方式 B：临时 access key

如果你必须使用 access key，请在 IAM 中创建专用用户或角色，并只授予部署所需权限。不要把 key 写入仓库。

配置：

```powershell
aws configure --profile energylossplus-dev
```

验证：

```powershell
aws sts get-caller-identity --profile energylossplus-dev
```

部署时使用：

```powershell
$env:AWS_PROFILE = "energylossplus-dev"
```

## 3. 安装依赖

在项目根目录执行：

```powershell
npm install
```

如果 Rust 依赖还没有下载，第一次测试或构建时 Cargo 会自动拉取。

## 4. 本地验证

推荐先跑完整验证：

```powershell
cargo test --workspace
npm test
npm run build
npm --workspace infra run build
npm audit --audit-level=moderate
```

桌面安装包构建：

```powershell
npm --workspace apps/desktop run tauri:build
```

成功后 Windows 安装包位于：

```text
target\release\bundle\msi\EnergyLossPlus_0.1.0_x64_en-US.msi
target\release\bundle\nsis\EnergyLossPlus_0.1.0_x64-setup.exe
```

## 5. CDK Synth 和部署

确保 Docker 正在运行，然后执行：

```powershell
npm --workspace infra run synth
```

如果这是第一次在该 AWS 账号/区域使用 CDK，需要 bootstrap：

```powershell
npx cdk bootstrap --profile energylossplus-dev
```

部署：

```powershell
npm run infra:deploy
```

或者显式指定 profile：

```powershell
$env:AWS_PROFILE = "energylossplus-dev"
npm run infra:deploy
```

部署成功后，记录 CDK 输出里的 `ApiUrl`，它是客户端需要访问的 API Gateway 地址。

## 6. Passkey / WebAuthn 配置

后端 Lambda 需要这些环境配置：

- `WEBAUTHN_RP_ID`：Relying Party ID，只写主机名，不带协议。例如 `app.example.com` 或开发时的 `localhost`。
- `WEBAUTHN_RP_NAME`：显示名称，例如 `EnergyLossPlus`。
- `WEBAUTHN_ORIGIN`：完整 origin，例如 `https://app.example.com` 或 `http://localhost:1420`。

注意：Passkey 对 origin 很严格。客户端实际运行的 origin 必须和后端配置一致，否则注册/登录验证会失败。

当前 CDK stack 已提供这些参数。需要部署到正式域名时，在 CDK 参数中传入对应值。

## 7. 配置客户端 API 地址

部署后，把 CDK 输出的 `ApiUrl` 配置给客户端。

React Passkey 请求使用：

```powershell
$env:VITE_API_BASE_URL = "https://your-api-id.execute-api.your-region.amazonaws.com"
```

Tauri Rust API client 使用：

```powershell
$env:ENERGY_API_BASE_URL = "https://your-api-id.execute-api.your-region.amazonaws.com"
```

开发模式启动：

```powershell
npm run tauri:dev
```

打包前也需要确保这些环境变量可用：

```powershell
npm --workspace apps/desktop run tauri:build
```

## 8. 部署后端到端检查

部署完成后，手动验证：

1. 打开 Tauri 应用。
2. 使用昵称 + 设备名注册 Passkey。
3. 退出后使用同一昵称登录。
4. 在目标页设置身高、体重、年龄、活动水平和目标类型。
5. 添加饮食记录。
6. 添加运动记录。
7. 添加体重记录。
8. 编辑和删除一条记录。
9. 重启应用，确认数据能从云端或本地缓存恢复。
10. 断网启动，确认只读缓存提示符合预期。

## 9. 常见问题

### `cdk synth` 提示 `spawnSync docker ENOENT`

Docker 没有安装、没有启动，或 `docker` 不在 PATH。安装并启动 Docker Desktop 后重开 PowerShell 再试。

### Tauri build 提示找不到 `cargo`

当前 PowerShell PATH 没有 Rust：

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

### npm 提示找不到 node/npm

当前 PowerShell PATH 没有 nvm 当前 Node 版本：

```powershell
$env:Path = "$env:LOCALAPPDATA\nvm\v26.3.0;$env:Path"
```

### Passkey 注册或登录失败

优先检查：

- `WEBAUTHN_RP_ID` 是否是裸主机名。
- `WEBAUTHN_ORIGIN` 是否和客户端实际 origin 完全一致。
- API Gateway 地址是否正确配置到 `VITE_API_BASE_URL` 和 `ENERGY_API_BASE_URL`。
- 当前系统/浏览器/WebView 是否支持 Passkey。

### AWS 权限不足

检查当前身份：

```powershell
aws sts get-caller-identity --profile energylossplus-dev
```

然后确认该身份有部署 CDK stack 所需权限，包括 CloudFormation、IAM、Lambda、API Gateway、DynamoDB、S3/CDK assets。

