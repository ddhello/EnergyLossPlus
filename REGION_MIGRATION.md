# AWS 区域迁移指南

本指南用于将 EnergyLossPlus 从美国东部区域迁移到东京区域，同时保留现有账号和全部业务数据。

- 源区域：`us-east-1`
- 目标区域：`ap-northeast-1`
- AWS CLI profile：`energy-profile`
- CloudFormation Stack：`EnergyLossPlusStack`

## 重要说明

迁移脚本会复制 DynamoDB 中的全部记录，包括账号、昵称索引、快照、饮食记录和临时认证记录。对于旧版 challenge、token 和 app-code 记录，脚本还会补充缺失的 TTL 属性。

API Gateway 地址包含区域，因此迁移后 WebAuthn RP ID 会改变。旧 Passkey 无法直接登录东京区域的新 API，但可以使用项目内置的“恢复原账号”功能，在保留原 `userId` 和全部数据的情况下注册新 Passkey。

在完成登录、数据同步、写入和账号恢复验证前，不要删除 `us-east-1` 中的旧资源。

## 1. 检查 AWS Profile

当前约定使用 `energy-profile`：

```powershell
$env:AWS_PROFILE = "energy-profile"
aws configure list-profiles
aws sts get-caller-identity --profile $env:AWS_PROFILE --region ap-northeast-1
```

如果提示找不到 `energy-profile`，需要先重新配置或登录：

```powershell
aws configure sso --profile energy-profile
aws sso login --profile energy-profile
```

如果该 profile 使用普通访问密钥而不是 SSO：

```powershell
aws configure --profile energy-profile
```

## 2. 构建并部署东京区域资源

构建 Lambda：

```powershell
npm run api:build
```

查询 AWS Account ID，并首次引导东京区域的 CDK：

```powershell
$accountId = aws sts get-caller-identity --profile $env:AWS_PROFILE --query Account --output text
npx cdk bootstrap "aws://$accountId/ap-northeast-1" --profile $env:AWS_PROFILE
```

部署东京区域 Stack：

```powershell
npm run infra:deploy
```

该 Stack 会在 `ap-northeast-1` 创建新的 Lambda、DynamoDB 和 API Gateway，并为 DynamoDB 启用 TTL。

## 3. 获取新 API 地址并配置 WebAuthn

获取东京区域 API Gateway 地址：

```powershell
$apiUrl = aws cloudformation describe-stacks `
  --stack-name EnergyLossPlusStack `
  --region ap-northeast-1 `
  --profile $env:AWS_PROFILE `
  --query "Stacks[0].Outputs[?OutputKey=='ApiUrl'].OutputValue | [0]" `
  --output text

$rpId = ([uri]$apiUrl).Host
$apiUrl
$rpId
```

重新部署，使 WebAuthn 使用新 API host：

```powershell
npx cdk deploy EnergyLossPlusStack `
  --region ap-northeast-1 `
  --profile $env:AWS_PROFILE `
  --parameters "EnergyLossPlusStack:WebauthnOrigin=$apiUrl" `
  --parameters "EnergyLossPlusStack:WebauthnRpId=$rpId"
```

## 4. 迁移 DynamoDB 数据

执行全量迁移：

```powershell
.\scripts\migrate-dynamodb-region.ps1 -Profile $env:AWS_PROFILE
```

脚本会：

- 自动查找源区域和目标区域的 DynamoDB 表名。
- 拒绝向非空目标表写入，防止意外混合数据。
- 分页扫描源表，并以每批最多 25 条写入目标表。
- 自动重试未处理的批量写入。
- 为旧临时认证记录补充 TTL。
- 完成后精确统计并比较源表和目标表记录数量。

如果目标表确实已经包含可安全覆盖或合并的数据，人工确认后才能使用：

```powershell
.\scripts\migrate-dynamodb-region.ps1 `
  -Profile $env:AWS_PROFILE `
  -AllowNonEmptyDestination
```

## 5. 恢复原账号并继承数据

生成临时恢复密钥：

```powershell
$recoveryKey = [Convert]::ToBase64String(
  [Security.Cryptography.RandomNumberGenerator]::GetBytes(32)
)
$recoveryKey
```

将临时恢复密钥部署到东京区域：

```powershell
npx cdk deploy EnergyLossPlusStack `
  --region ap-northeast-1 `
  --profile $env:AWS_PROFILE `
  --parameters "EnergyLossPlusStack:WebauthnOrigin=$apiUrl" `
  --parameters "EnergyLossPlusStack:WebauthnRpId=$rpId" `
  --parameters "EnergyLossPlusStack:PasskeyRecoveryKey=$recoveryKey"
```

然后进行账号恢复：

1. 打开已指向东京区域 API 的 EnergyLossPlus。
2. 输入原来的用户名。
3. 点击“恢复原账号”。
4. 在外部 Passkey 页面输入 `$recoveryKey`。
5. 创建新的 Passkey。
6. 登录后确认原来的每日目标和饮食记录仍然存在。

恢复操作会替换原账号的旧 Passkey，但会保留原 `userId` 和关联快照，因此数据不会丢失。

恢复成功后，立即清空恢复密钥以关闭恢复入口：

```powershell
npx cdk deploy EnergyLossPlusStack `
  --region ap-northeast-1 `
  --profile $env:AWS_PROFILE `
  --parameters "EnergyLossPlusStack:WebauthnOrigin=$apiUrl" `
  --parameters "EnergyLossPlusStack:WebauthnRpId=$rpId" `
  --parameters "EnergyLossPlusStack:PasskeyRecoveryKey="
```

## 6. 将客户端切换到东京区域

本地构建：

```powershell
$env:VITE_API_BASE_URL = $apiUrl
$env:ENERGY_API_BASE_URL = $apiUrl
npm run build
```

同时将 GitHub Actions 仓库变量 `API_BASE_URL` 更新为 `$apiUrl`。

## 7. 最终验证清单

- 新 API 地址包含 `execute-api.ap-northeast-1.amazonaws.com`。
- 源表和目标表记录数量一致。
- 使用原用户名完成“恢复原账号”。
- 登录后可以看到原有每日热量目标和饮食记录。
- 可以新增和删除记录。
- 另一台设备同步后可以看到相同数据。
- DynamoDB TTL 已启用，临时认证记录包含 `expiresAtEpoch`。
- 已清空 `PasskeyRecoveryKey`。

全部验证通过后，再考虑停止或删除 `us-east-1` 中的旧资源。
