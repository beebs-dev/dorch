FROM node:20-alpine AS deps
WORKDIR /app

COPY browser/package.json browser/package-lock.json ./
RUN npm ci


FROM node:20-alpine AS build
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY browser/ .

ENV NODE_ENV=production
RUN npm run build


FROM node:20-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

# Install only production deps
COPY browser/package.json browser/package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force

COPY --from=build /app/build ./build
# Lua script for Redis-backed rate limiter (loaded at runtime)
COPY common/src/rate_limit/rate_limit.lua ./rate_limit.lua
#COPY --from=build /app/icons ./icons

EXPOSE 3000
CMD ["node", "build"]
