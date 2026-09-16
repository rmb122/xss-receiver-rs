#!/bin/sh

## TIPS: 必须从仓库根目录运行

IP2REGION_VERSION=v3.18.0
IP2REGION_BASE_URL="https://raw.githubusercontent.com/lionsoul2014/ip2region/refs/tags/${IP2REGION_VERSION}/data"

curl -fsSL "${IP2REGION_BASE_URL}/ip2region_v4.xdb" -o docker/ip2region_v4.xdb
curl -fsSL "${IP2REGION_BASE_URL}/ip2region_v6.xdb" -o docker/ip2region_v6.xdb
