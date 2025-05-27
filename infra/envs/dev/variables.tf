variable "aws_region" {
  description = "AWS region to deploy into"
  type        = string
  default     = "us-east-1"
}

variable "ssh_keypair" {
  description = "Name of an existing EC2 key pair"
  type        = string
}

variable "yb_version" {
  description = "Yugabyte version / AMI tag"
  type        = string
  default     = "2024.2.3"
}
