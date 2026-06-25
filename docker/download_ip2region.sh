#!/bin/sh

## TIPS: 必须从仓库根目录运行

curl -fsSL https://raw.githubusercontent.com/lionsoul2014/ip2region/refs/heads/master/data/ip2region_v4.xdb -o docker/ip2region_v4.xdb
curl -fsSL https://raw.githubusercontent.com/lionsoul2014/ip2region/refs/heads/master/data/ip2region_v6.xdb -o docker/ip2region_v6.xdb
