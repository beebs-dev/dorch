
# Build and run SvelteKit (adapter-node) inside a container.
#
# Build:
#   docker build -t psyslop-landing .
# Run:
#   docker run --rm -p 3000:3000 --env-file .env psyslop-landing

FROM node:20-alpine AS deps
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci


FROM node:20-alpine AS build
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY . .

ENV NODE_ENV=production
RUN npm run build


FROM node:20-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

# Install only production deps
COPY package.json package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force

COPY --from=build /app/build ./build
#COPY --from=build /app/icons ./icons

EXPOSE 3000
CMD ["node", "build"]
