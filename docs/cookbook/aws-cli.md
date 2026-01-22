# AWS CLI Integration

Build terminal-based AWS resource browsers using the AWS CLI and TermStack.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Complete Configuration](#complete-configuration)
- [Usage Instructions](#usage-instructions)
- [Features](#features)
- [Security Notes](#security-notes)
- [Troubleshooting](#troubleshooting)

---

## Overview

This example demonstrates how to browse AWS resources (S3, EC2, Lambda) using TermStack and the AWS CLI. It includes:

- **S3 bucket browser** with object listing
- **EC2 instance dashboard** with status indicators
- **Lambda function viewer**
- **Multi-region support**
- **Profile-based authentication**

**What You'll Build:**
```
Main Menu → S3 Buckets → Objects in Bucket
         → EC2 Instances
         → Lambda Functions
```

---

## Prerequisites

### 1. AWS CLI Installation

```bash
# macOS
brew install awscli

# Linux
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install

# Verify installation
aws --version
```

### 2. Configure AWS Credentials

```bash
# Interactive configuration
aws configure

# You'll be prompted for:
# AWS Access Key ID: AKIAIOSFODNN7EXAMPLE
# AWS Secret Access Key: wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
# Default region: us-west-2
# Default output format: json
```

**Credentials are stored in:** `~/.aws/credentials`
```ini
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[production]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
```

### 3. Verify Access

```bash
# Test credentials
aws sts get-caller-identity

# List S3 buckets (if you have any)
aws s3 ls

# List EC2 instances
aws ec2 describe-instances --output json
```

---

## Complete Configuration

Save this as `aws-resources.yaml`:

```yaml
version: v1

app:
  name: "AWS Resource Browser"
  description: "Browse S3, EC2, Lambda via AWS CLI"
  theme: "default"

globals:
  # AWS CLI uses ~/.aws/credentials automatically
  # Override profile if needed
  aws_profile: "{{ env.AWS_PROFILE | default(value='default') }}"
  aws_region: "{{ env.AWS_REGION | default(value='us-west-2') }}"

start: main_menu

pages:
  # ==========================================================================
  # MAIN MENU - Resource selection
  # ==========================================================================
  main_menu:
    title: "AWS Resources"
    description: "Select a resource type to browse"
    
    data:
      adapter: cli
      command: "echo"
      args: ['[{"name":"S3 Buckets","key":"s3","desc":"Object storage"},{"name":"EC2 Instances","key":"ec2","desc":"Virtual machines"},{"name":"Lambda Functions","key":"lambda","desc":"Serverless functions"}]']
      items: "$[*]"
    
    view:
      type: table
      columns:
        - path: "$.name"
          display: "Resource Type"
          width: 30
          style:
            - default: true
              color: cyan
              bold: true
        - path: "$.desc"
          display: "Description"
          width: 50
    
    next:
      - condition: "{{ row.key == 's3' }}"
        page: s3_buckets
      - condition: "{{ row.key == 'ec2' }}"
        page: ec2_instances
      - condition: "{{ row.key == 'lambda' }}"
        page: lambda_functions

  # ==========================================================================
  # S3 BUCKETS - List all S3 buckets
  # ==========================================================================
  s3_buckets:
    title: "S3 Buckets"
    description: "All S3 buckets in your AWS account"
    
    data:
      adapter: cli
      command: "aws"
      args: ["s3api", "list-buckets", "--output", "json"]
      items: "$.Buckets[*]"
      timeout: "30s"
      refresh_interval: "5m"
      env:
        AWS_PROFILE: "{{ aws_profile }}"
    
    view:
      type: table
      columns:
        - path: "$.Name"
          display: "Bucket Name"
          width: 60
          style:
            - default: true
              color: cyan
              bold: true
        - path: "$.CreationDate"
          display: "Created"
          width: 30
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray
    
    next:
      page: s3_objects
      context:
        bucket_name: "$.Name"

  # ==========================================================================
  # S3 OBJECTS - List objects in selected bucket
  # ==========================================================================
  s3_objects:
    title: "Objects - {{ s3_buckets.Name }}"
    description: "Contents of bucket {{ s3_buckets.Name }}"
    
    data:
      adapter: cli
      command: "aws"
      args: ["s3api", "list-objects-v2", "--bucket", "{{ s3_buckets.Name }}", "--output", "json"]
      items: "$.Contents[*]"
      timeout: "30s"
      env:
        AWS_PROFILE: "{{ aws_profile }}"
    
    view:
      type: table
      columns:
        - path: "$.Key"
          display: "Object Key"
          width: 70
          style:
            - default: true
              color: white
        - path: "$.Size"
          display: "Size"
          width: 15
          transform: "{{ value | filesizeformat }}"
          style:
            - default: true
              color: cyan
        - path: "$.LastModified"
          display: "Modified"
          width: 25
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray

  # ==========================================================================
  # EC2 INSTANCES - List EC2 instances
  # ==========================================================================
  ec2_instances:
    title: "EC2 Instances"
    description: "EC2 instances in {{ aws_region }}"
    
    data:
      adapter: cli
      command: "aws"
      args: ["ec2", "describe-instances", "--output", "json"]
      items: "$.Reservations[*].Instances[*]"
      timeout: "30s"
      refresh_interval: "2m"
      env:
        AWS_PROFILE: "{{ aws_profile }}"
        AWS_REGION: "{{ aws_region }}"
    
    view:
      type: table
      columns:
        - path: "$.InstanceId"
          display: "Instance ID"
          width: 20
          style:
            - default: true
              color: cyan
        - path: "$.InstanceType"
          display: "Type"
          width: 15
          style:
            - default: true
              color: magenta
        - path: "$.State.Name"
          display: "State"
          width: 12
          style:
            - condition: "{{ value == 'running' }}"
              color: green
              bold: true
            - condition: "{{ value == 'stopped' }}"
              color: red
            - condition: "{{ value == 'pending' }}"
              color: yellow
            - default: true
              color: gray
        - path: "$.PublicIpAddress"
          display: "Public IP"
          width: 16
          style:
            - default: true
              color: cyan
        - path: "$.PrivateIpAddress"
          display: "Private IP"
          width: 16
          style:
            - default: true
              color: blue
        - path: "$.LaunchTime"
          display: "Launched"
          width: 20
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray

  # ==========================================================================
  # LAMBDA FUNCTIONS - List Lambda functions
  # ==========================================================================
  lambda_functions:
    title: "Lambda Functions"
    description: "Lambda functions in {{ aws_region }}"
    
    data:
      adapter: cli
      command: "aws"
      args: ["lambda", "list-functions", "--output", "json"]
      items: "$.Functions[*]"
      timeout: "30s"
      refresh_interval: "5m"
      env:
        AWS_PROFILE: "{{ aws_profile }}"
        AWS_REGION: "{{ aws_region }}"
    
    view:
      type: table
      columns:
        - path: "$.FunctionName"
          display: "Function"
          width: 40
          style:
            - default: true
              color: cyan
              bold: true
        - path: "$.Runtime"
          display: "Runtime"
          width: 15
          style:
            - default: true
              color: magenta
        - path: "$.MemorySize"
          display: "Memory (MB)"
          width: 12
          style:
            - default: true
              color: yellow
        - path: "$.Timeout"
          display: "Timeout (s)"
          width: 12
          style:
            - default: true
              color: blue
        - path: "$.LastModified"
          display: "Modified"
          width: 25
          transform: "{{ value | timeago }}"
          style:
            - default: true
              color: gray
```

---

## Usage Instructions

### 1. Default Profile

```bash
# Uses [default] profile from ~/.aws/credentials
termstack aws-resources.yaml
```

### 2. Specific Profile

```bash
# Uses [production] profile
AWS_PROFILE=production termstack aws-resources.yaml
```

### 3. Specific Region

```bash
# Override region
AWS_REGION=eu-west-1 termstack aws-resources.yaml
```

### 4. Both Profile and Region

```bash
AWS_PROFILE=production AWS_REGION=us-east-1 termstack aws-resources.yaml
```

---

## Features

### S3 Browser
- ✅ List all buckets
- ✅ Browse objects in bucket
- ✅ File size formatting
- ✅ Time formatting

### EC2 Dashboard
- ✅ Instance listing
- ✅ Status indicators (running/stopped/pending)
- ✅ Public and private IPs
- ✅ Instance types
- ✅ Launch times

### Lambda Viewer
- ✅ Function listing
- ✅ Runtime information
- ✅ Memory and timeout configuration
- ✅ Last modified times

### General
- ✅ **Profile Support**: Switch AWS profiles
- ✅ **Region Support**: Change AWS regions
- ✅ **Auto-refresh**: Configurable refresh intervals
- ✅ **Styling**: Color-coded statuses

---

## Security Notes

### ✅ DO

**Use IAM Roles** (recommended for EC2/Lambda):
```bash
# No credentials needed - uses instance role
aws configure set region us-west-2
```

**Use Profiles**:
```bash
aws configure --profile readonly
# In YAML: env: AWS_PROFILE: "readonly"
```

**Use Read-Only Permissions**:
```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": [
      "s3:ListBucket",
      "s3:GetObject",
      "ec2:Describe*",
      "lambda:List*",
      "lambda:Get*"
    ],
    "Resource": "*"
  }]
}
```

**Rotate Keys**:
```bash
# Rotate AWS access keys every 90 days
aws iam create-access-key --user-name myuser
```

### ❌ DON'T

❌ Use root account credentials  
❌ Share credentials files  
❌ Commit credentials to git  
❌ Use admin permissions for read-only tasks

---

## Troubleshooting

### AWS CLI Not Found

```bash
which aws
# If empty, install AWS CLI
brew install awscli  # macOS
```

### Invalid Credentials

```bash
# Check configuration
aws configure list

# Test credentials
aws sts get-caller-identity

# Reconfigure if needed
aws configure
```

### Permission Denied

```bash
# Check IAM permissions
aws iam get-user

# Verify you have required permissions:
# - s3:ListAllMyBuckets, s3:ListBucket
# - ec2:DescribeInstances
# - lambda:ListFunctions
```

### Wrong Region

```bash
# Check current region
aws configure get region

# Override region
AWS_REGION=us-east-1 termstack aws-resources.yaml
```

### Empty Results

```bash
# Verify resources exist
aws s3 ls
aws ec2 describe-instances
aws lambda list-functions

# Check you're in the right region
aws ec2 describe-instances --region us-west-2
```

---

## See Also

- [Authentication Guide](../guides/authentication.md) - Comprehensive auth documentation
- [Templates & Context Guide](../guides/templates-and-context.md) - Template syntax and navigation
- [GitHub API Example](github-api.md) - Similar example for HTTP APIs
- [Documentation Hub](../README.md) - Central documentation index
- [AWS CLI Documentation](https://docs.aws.amazon.com/cli/) - Official AWS CLI docs
