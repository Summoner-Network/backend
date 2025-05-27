###############################################################################
# Bootstrap remote backend: S3 bucket + DynamoDB lock table
###############################################################################

provider "aws" {
  region = "us-east-1"
}

resource "aws_s3_bucket" "tf_state" {
  bucket = var.state_bucket
  force_destroy = false

  versioning {
    enabled = true
  }

  server_side_encryption_configuration {
    rule {
      apply_server_side_encryption_by_default {
        sse_algorithm = "AES256"
      }
    }
  }

  tags = {
    Project = "infra"
    Terraform = "state"
  }
}

resource "aws_dynamodb_table" "tf_lock" {
  name         = var.lock_table
  billing_mode = "PAY_PER_REQUEST"

  hash_key = "LockID"

  attribute {
    name = "LockID"
    type = "S"
  }

  tags = {
    Project = "infra"
    Terraform = "lock"
  }
}

# ── Outputs you’ll need in backend blocks ───────────────
output "bucket_name" { value = aws_s3_bucket.tf_state.id }
output "table_name"  { value = aws_dynamodb_table.tf_lock.name }
