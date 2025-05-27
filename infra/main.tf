// ==============================
//  main.tf – Fargate ALB stack
// ==============================

terraform {
  required_version = ">= 1.6"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.50"
    }
  }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project   = var.project_name
      ManagedBy = "terraform"
      # add Owner, CostCenter, etc. if your org uses them
    }
  }
}


#--------------------------------------------------
#  Availability zones (first two in region)
#--------------------------------------------------
data "aws_availability_zones" "available" {}

#--------------------------------------------------
#  VPC: 2 public + 2 private subnets + 1 NAT
#--------------------------------------------------
module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"

  name = "${var.project_name}-vpc"
  cidr = "10.0.0.0/16"

  azs             = slice(data.aws_availability_zones.available.names, 0, 2)
  public_subnets  = ["10.0.0.0/24", "10.0.1.0/24"]
  private_subnets = ["10.0.8.0/21", "10.0.16.0/21"]

  enable_nat_gateway = true
  single_nat_gateway = true
}

#--------------------------------------------------
#  ECR repo & CloudWatch log group
#--------------------------------------------------
resource "aws_ecr_repository" "repo" {
  name                 = var.project_name
  image_tag_mutability = "MUTABLE"
}

resource "aws_cloudwatch_log_group" "ecs" {
  name              = "/ecs/${var.project_name}"
  retention_in_days = 14
}

#--------------------------------------------------
#  ECS cluster & execution role
#--------------------------------------------------
module "ecs" {
  source  = "terraform-aws-modules/ecs/aws"
  version = "~> 5.0"

  cluster_name = "${var.project_name}-cluster"
}

data "aws_iam_policy_document" "ecs_exec_assume" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "exec" {
  name               = "${var.project_name}-exec-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_exec_assume.json
}

resource "aws_iam_role_policy_attachment" "exec_policy" {
  role       = aws_iam_role.exec.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

#--------------------------------------------------
#  ECS task definition (Fargate)
#--------------------------------------------------
data "aws_ecr_repository" "repo_url" {
  name = aws_ecr_repository.repo.name
}

resource "aws_ecs_task_definition" "web" {
  family                   = var.project_name
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.cpu
  memory                   = var.memory
  execution_role_arn       = aws_iam_role.exec.arn

  container_definitions = jsonencode([
    {
      name      = "web"
      image     = "${data.aws_ecr_repository.repo_url.repository_url}:${var.container_tag}"
      essential = true

      portMappings = [
        { containerPort = 80, protocol = "tcp" }
      ]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = aws_cloudwatch_log_group.ecs.name
          awslogs-region        = var.aws_region
          awslogs-stream-prefix = "ecs"
        }
      }
    }
  ])
}

#--------------------------------------------------
#  Security groups
#--------------------------------------------------
resource "aws_security_group" "alb_sg" {
  name   = "${var.project_name}-alb-sg"
  vpc_id = module.vpc.vpc_id

  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_security_group" "task_sg" {
  name   = "${var.project_name}-task-sg"
  vpc_id = module.vpc.vpc_id

  ingress {
    from_port       = 80
    to_port         = 80
    protocol        = "tcp"
    security_groups = [aws_security_group.alb_sg.id]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

#--------------------------------------------------
#  Load balancer, target group & listener
#--------------------------------------------------
# --- Application Load Balancer ---------------------------------------------
resource "aws_lb" "alb" {
  name               = "${var.project_name}-alb"
  load_balancer_type = "application"
  subnets            = module.vpc.public_subnets
  security_groups    = [aws_security_group.alb_sg.id]

  lifecycle {
    create_before_destroy = true   # keep old LB until new one is ready
  }
}

# --- Target Group -----------------------------------------------------------
resource "aws_lb_target_group" "tg" {
  name        = "${var.project_name}-tg"
  port        = 80
  protocol    = "HTTP"
  vpc_id      = module.vpc.vpc_id
  target_type = "ip"

  health_check {
    path                = "/index.html"
    matcher             = "200-399"
    interval            = 15
    timeout             = 5
    healthy_threshold   = 2
    unhealthy_threshold = 2
  }

  lifecycle {
    create_before_destroy = true   # same logic for TG
  }
}

resource "aws_lb_listener" "http" {
  load_balancer_arn = aws_lb.alb.arn
  port              = 80
  protocol          = "HTTP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.tg.arn
  }
}

#--------------------------------------------------
#  ECS Fargate service
#--------------------------------------------------
resource "aws_ecs_service" "svc" {
  name            = "${var.project_name}-svc"
  cluster         = module.ecs.cluster_id
  task_definition = aws_ecs_task_definition.web.arn
  desired_count   = 1
  launch_type     = "FARGATE"

  network_configuration {
    subnets         = module.vpc.private_subnets
    security_groups = [aws_security_group.task_sg.id]
    assign_public_ip = false
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.tg.arn
    container_name   = "web"
    container_port   = 80
  }

  lifecycle {
    ignore_changes = [desired_count]
  }
}

# target == the ECS service’s desired_count
resource "aws_appautoscaling_target" "ecs_svc" {
  service_namespace  = "ecs"
  resource_id        = "service/${module.ecs.cluster_name}/${aws_ecs_service.svc.name}"
  scalable_dimension = "ecs:service:DesiredCount"

  min_capacity = 1
  max_capacity = 5

  depends_on = [aws_ecs_service.svc]  # optional safety net
}


# simple target-tracking: keep CPU at ~60 %
resource "aws_appautoscaling_policy" "cpu_target" {
  name               = "${var.project_name}-cpu-policy"
  policy_type        = "TargetTrackingScaling"
  resource_id        = aws_appautoscaling_target.ecs_svc.resource_id
  scalable_dimension = aws_appautoscaling_target.ecs_svc.scalable_dimension
  service_namespace  = aws_appautoscaling_target.ecs_svc.service_namespace

  target_tracking_scaling_policy_configuration {
    predefined_metric_specification {
      predefined_metric_type = "ECSServiceAverageCPUUtilization"
    }
    target_value       = 60
    scale_in_cooldown  = 60  # seconds
    scale_out_cooldown = 60
  }
}
