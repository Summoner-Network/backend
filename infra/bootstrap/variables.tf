variable "state_bucket" {
  type        = string
  default     = "summoner-terraform-state"   # globally unique bucket name
}

variable "lock_table" {
  type        = string
  default     = "tf-locks"
}
