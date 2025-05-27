variable "cluster_name"       { type = string }
variable "cluster_size"       { type = number }
variable "replication_factor" { type = number }
variable "instance_type"      { type = string }

variable "aws_region"   { type = string }

# Optional networking overrides (defaults let dev/prod omit them)
variable "vpc_id" {
  type    = string
  default = ""
}

variable "public_subnets" {
  type    = list(string)
  default = []
}

variable "ssh_keypair" {
  type    = string
  default = ""
}

variable "yb_version" { type = string }

variable "ysql_password" {
  type      = string
  sensitive = true
  default   = ""   # empty = auth disabled
}