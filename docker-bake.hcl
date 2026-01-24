variable "REGISTRY" {
  default = ""
}

group "default" {
  targets = [
    "operator",
    "server",
    "proxy",
    "client",
    "sock",
    "iam",
    "wadinfo",
    "master",
    "webrtc-auth",
    "party",
    "auth",
    "browser",
    "archiver",
    "analyzer",
    "spectator",
    "downloader"
  ]
}

group "archiver" {
  targets = ["archiver-worker", "archiver-init"]
}

target "base" {
  context    = "./"
  dockerfile = "Dockerfile.base"
  tags       = ["${REGISTRY}thavlik/dorch-base:latest"]
  push       = false
}

target "browser" {
  context    = "./"
  dockerfile = "browser/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-browser:latest"]
  push       = true
}

target "wadinfo" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "wadinfo/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-wadinfo:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-wadinfo"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-wadinfo,mode=min"]
}

target "analyzer" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "analyzer/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-analyzer:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-analyzer"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-analyzer,mode=min"]
}

target "archiver-worker" {
  context    = "./"
  dockerfile = "archiver/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-archiver:latest"]
  push       = true
}

target "auth" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  args       = { BASE_IMAGE = "base_context" }
  dockerfile = "auth/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-auth:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-auth"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-auth,mode=min"]
}

target "archiver-init" {
  context    = "./"
  dockerfile = "archiver/Dockerfile.init"
  tags       = ["${REGISTRY}thavlik/dorch-archiver-init:latest"]
  push       = true
}

target "master" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  args       = { BASE_IMAGE = "base_context" }
  dockerfile = "master/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-master:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-master"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-master,mode=min"]
}

target "operator" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "operator/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-operator:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-operator"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-operator,mode=min"]
}


target "iam" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "iam/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-iam:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-iam"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-iam,mode=min"]
}

target "party" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "party/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-party:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-party"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-party,mode=min"]
}

target "webrtc-auth" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "webrtc-auth/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-webrtc-auth:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-webrtc-auth"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-webrtc-auth,mode=min"]
}

target "sock" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "sock/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-sock:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-sock"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-sock,mode=min"]
}

target "server" {
  context    = "zandronum/"
  dockerfile = "Dockerfile.server"
  tags       = ["${REGISTRY}thavlik/zandronum-server:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-server"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-server,mode=min"]
}

target "spectator" {
  context    = "zandronum/"
  dockerfile = "Dockerfile.spectator"
  tags       = ["${REGISTRY}thavlik/zandronum-spectator:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-spectator"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-spectator,mode=min"]
}

target "downloader" {
  context    = "downloader/"
  tags       = ["${REGISTRY}thavlik/dorch-downloader:latest"]
  push       = true
}

target "proxy" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "proxy/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-proxy:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-proxy"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-proxy,mode=min"]
}

target "client" {
  context    = "zandronum/"
  dockerfile = "Dockerfile.client"
  tags       = ["${REGISTRY}thavlik/zandronum-client:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/dorch-client"]
  cache-to   = ["type=local,dest=.buildx-cache/dorch-client,mode=min"]
}
