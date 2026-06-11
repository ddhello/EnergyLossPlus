import { CfnOutput, CfnParameter, Duration, RemovalPolicy, Stack, StackProps } from "aws-cdk-lib";
import { CorsHttpMethod, HttpApi, HttpMethod } from "aws-cdk-lib/aws-apigatewayv2";
import { HttpLambdaIntegration } from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { AttributeType, BillingMode, Table } from "aws-cdk-lib/aws-dynamodb";
import { Architecture, Code, Function, Runtime } from "aws-cdk-lib/aws-lambda";
import { Construct } from "constructs";
import { join } from "node:path";

const webOrigins = [
  "https://energylossplus.erasereat.workers.dev",
  "https://energy.114522.xyz",
  "https://energy.mipa.moe"
];

export class EnergyLossPlusStack extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);

    const webauthnRpId = new CfnParameter(this, "WebauthnRpId", {
      type: "String",
      default: "localhost",
      description: "Bare WebAuthn relying-party host, for example app.example.com."
    });
    const webauthnOrigin = new CfnParameter(this, "WebauthnOrigin", {
      type: "String",
      default: "http://localhost:1420",
      description: "HTTPS origin hosting the external browser Passkey page."
    });
    const webauthnRpName = new CfnParameter(this, "WebauthnRpName", {
      type: "String",
      default: "EnergyLossPlus",
      description: "Display name shown by platform Passkey prompts."
    });

    const table = new Table(this, "DataTable", {
      partitionKey: { name: "pk", type: AttributeType.STRING },
      sortKey: { name: "sk", type: AttributeType.STRING },
      billingMode: BillingMode.PAY_PER_REQUEST,
      timeToLiveAttribute: "expiresAtEpoch",
      removalPolicy: RemovalPolicy.RETAIN,
      pointInTimeRecoverySpecification: {
        pointInTimeRecoveryEnabled: true
      }
    });

    const apiFunction = new Function(this, "ApiFunction", {
      runtime: Runtime.PROVIDED_AL2023,
      architecture: Architecture.ARM_64,
      handler: "bootstrap",
      code: Code.fromAsset(join(process.cwd(), "..", "target", "lambda", "energy-api")),
      timeout: Duration.seconds(15),
      memorySize: 512,
      environment: {
        TABLE_NAME: table.tableName,
        RUST_LOG: "info",
        WEBAUTHN_RP_ID: webauthnRpId.valueAsString,
        WEBAUTHN_RP_NAME: webauthnRpName.valueAsString,
        WEBAUTHN_ORIGIN: webauthnOrigin.valueAsString,
        WEB_ORIGINS: webOrigins.join(",")
      }
    });

    table.grantReadWriteData(apiFunction);

    const httpApi = new HttpApi(this, "HttpApi", {
      corsPreflight: {
        allowHeaders: ["Content-Type", "Authorization"],
        allowMethods: [
          CorsHttpMethod.OPTIONS,
          CorsHttpMethod.GET,
          CorsHttpMethod.POST,
          CorsHttpMethod.PUT,
          CorsHttpMethod.DELETE
        ],
        allowOrigins: [webauthnOrigin.valueAsString, ...webOrigins]
      }
    });
    const integration = new HttpLambdaIntegration("ApiIntegration", apiFunction);

    httpApi.addRoutes({
      path: "/{proxy+}",
      methods: [HttpMethod.ANY],
      integration
    });

    new CfnOutput(this, "ApiUrl", {
      value: httpApi.apiEndpoint,
      description: "EnergyLossPlus HTTP API base URL."
    });
    new CfnOutput(this, "DataTableName", {
      value: table.tableName,
      description: "DynamoDB table used by EnergyLossPlus."
    });
  }
}
