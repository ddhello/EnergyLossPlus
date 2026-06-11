import { App } from "aws-cdk-lib";
import { EnergyLossPlusStack } from "../lib/energy-loss-plus-stack";

const app = new App();

new EnergyLossPlusStack(app, "EnergyLossPlusStack", {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: "ap-northeast-1"
  }
});
