param(
    [string]$SourceRegion = "us-east-1",
    [string]$DestinationRegion = "ap-northeast-1",
    [string]$StackName = "EnergyLossPlusStack",
    [string]$Profile,
    [string]$SourceTableName,
    [string]$DestinationTableName,
    [switch]$AllowNonEmptyDestination
)

$ErrorActionPreference = "Stop"
$sourceTable = if ($SourceTableName) { $SourceTableName } else {
    aws cloudformation list-stack-resources --stack-name $StackName --region $SourceRegion --profile $Profile --query "StackResourceSummaries[?ResourceType=='AWS::DynamoDB::Table'].PhysicalResourceId | [0]" --output text
}
$destinationTable = if ($DestinationTableName) { $DestinationTableName } else {
    aws cloudformation list-stack-resources --stack-name $StackName --region $DestinationRegion --profile $Profile --query "StackResourceSummaries[?ResourceType=='AWS::DynamoDB::Table'].PhysicalResourceId | [0]" --output text
}
if ($LASTEXITCODE -ne 0 -or -not $sourceTable -or -not $destinationTable) {
    throw "Unable to resolve source or destination DynamoDB table."
}

$env:AWS_PROFILE = $Profile
$arguments = @(
    "run", "--quiet", "-p", "energy-api", "--bin", "migrate_dynamodb", "--",
    "--source-region", $SourceRegion,
    "--destination-region", $DestinationRegion,
    "--source-table", $sourceTable,
    "--destination-table", $destinationTable
)
if ($AllowNonEmptyDestination) { $arguments += "--allow-non-empty-destination" }

& cargo @arguments
if ($LASTEXITCODE -ne 0) {
    throw "DynamoDB region migration failed."
}
