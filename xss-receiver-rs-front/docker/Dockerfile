# 构建阶段
FROM node:22-alpine AS build

WORKDIR /app

# 先复制依赖文件，利用 Docker 缓存层
COPY package.json package-lock.json* ./

RUN npm config set registry https://registry.npmmirror.com
RUN npm install

# 复制项目源码并构建
COPY . .

RUN npm run build

# 运行阶段
FROM nginx:alpine

# 复制 nginx 配置
RUN rm /etc/nginx/conf.d/default.conf
COPY docker/nginx.conf /etc/nginx/templates/default.conf.template

# 复制构建产物到 nginx 默认目录
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
