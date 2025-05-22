variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "vpc_cidr" {
  description = "CIDR for the shared VPC"
  type        = string
  default     = "10.0.0.0/16"
}