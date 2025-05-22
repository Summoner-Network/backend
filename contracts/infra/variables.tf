variable "aws_region"      { default = "us-east-1" }
variable "project_name"    { default = "my-webapp" }
variable "container_tag"   { default = "latest" }   # you can override in CLI
variable "cpu"             { default = 256 }
variable "memory"          { default = 512 }
