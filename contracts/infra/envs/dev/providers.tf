terraform {
  required_version = ">= 1.6"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 3.76"
    }
  }
  backend "s3" {
    bucket         = "summoner-terraform-state"   # ← from output
    key            = "network/terraform.tfstate"    # or dev/prod/…
    region         = "us-east-1"
    dynamodb_table = "tf-locks"
    encrypt        = true
  }
}

provider "aws" {
  region = var.aws_region
}