##########################################################
# Yugabyte – DEV (single-node, no auth)
##########################################################

# Pull VPC info from the `network` state
data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "my-tf-state"
    key    = "network/terraform.tfstate"
    region = "us-east-1"
  }
}

module "yugabyte_dev" {
  source             = "../../modules/yugabyte"

  # ── sizing ────────────────────────────────────────────
  cluster_name        = "yb-dev"
  cluster_size        = 1
  replication_factor  = 1
  instance_type       = "t3.medium"

  # ── networking ───────────────────────────────────────
  vpc_id         = data.terraform_remote_state.network.outputs.vpc_id
  public_subnets = data.terraform_remote_state.network.outputs.public_subnet_ids

  # ── misc ─────────────────────────────────────────────
  aws_region     = var.aws_region
  ssh_keypair    = var.ssh_keypair
  yb_version     = var.yb_version
  ysql_password  = ""          # auth disabled in dev
}