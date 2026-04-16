#!/bin/sh

## TIPS: 必须从仓库根目录运行

sudo docker build -f docker/Dockerfile -t ccr.ccs.tencentyun.com/rmb122/xss-receiver-frontend:latest .
sudo docker push ccr.ccs.tencentyun.com/rmb122/xss-receiver-frontend:latest