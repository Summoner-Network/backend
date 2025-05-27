##########################################################
# Yugabyte – PROD (five-node, auth enabled)
##########################################################

# VPC from the shared network state
data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "my-tf-state"
    key    = "network/terraform.tfstate"
    region = "us-east-1"
  }
}

# prod password
data "aws_secretsmanager_secret" "yb_prod_pwd" {
  name = "yb/prod/password"
}

data "aws_secretsmanager_secret_version" "current" {
  secret_id = data.aws_secretsmanager_secret.yb_prod_pwd.id
}

module "yugabyte_prod" {
  source             = "../../modules/yugabyte"

  # sizing
  cluster_name        = "yb-prod"
  cluster_size        = 5
  replication_factor  = 3
  instance_type       = "m6i.large"

  # networking
  vpc_id         = data.terraform_remote_state.network.outputs.vpc_id
  public_subnets = data.terraform_remote_state.network.outputs.public_subnet_ids

  # auth & misc
  aws_region     = var.aws_region
  ssh_keypair    = var.ssh_keypair
  yb_version     = var.yb_version
  ysql_password  = data.aws_secretsmanager_secret_version.current.secret_string
}