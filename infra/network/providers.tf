terraform {
  required_version = ">= 1.6"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"      # any 5.x release, satisfies >= 5.20
    }
  }

  backend "s3" {
    bucket         = "summoner-terraform-state"
    key            = "network/terraform.tfstate"
    region         = "us-east-1"
    dynamodb_table = "tf-locks"
    encrypt        = true
  }
}

provider "aws" {
  region = var.aws_region
}
