###############################################################################
# Wrapper around github.com/yugabyte/terraform-aws-yugabyte
###############################################################################

module "yb" {
  source  = "github.com/yugabyte/terraform-aws-yugabyte"

  # ── naming & sizing ───────────────────────────────────
  cluster_name        = var.cluster_name
  num_instances       = var.cluster_size
  replication_factor  = var.replication_factor
  instance_type       = var.instance_type

  # ── networking ────────────────────────────────────────
  region_name = var.aws_region
  vpc_id      = var.vpc_id != "" ? var.vpc_id : data.aws_vpc.default.id
  subnet_ids  = length(var.public_subnets) != 0 ? var.public_subnets : data.aws_subnets.default_public.ids

  # ── auth & version ────────────────────────────────────
  yb_version     = var.yb_version
  ysql_password  = var.ysql_password
}
