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
    "downloader"
  ]
}

target "base" {
  context    = "./"
  dockerfile = "Dockerfile.base"
  tags       = ["${REGISTRY}thavlik/dorch-base:latest"]
  push       = false
}

target "wadinfo" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "wadinfo/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-wadinfo:latest"]
  push       = true
  cache-from = ["type=local,src=.buildx-cache/wadinfo"]
  cache-to   = ["type=local,dest=.buildx-cache/wadinfo,mode=min"]
}

target "archiver" {
  context    = "./"
  dockerfile = "archiver/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-archiver:latest"]
  push       = true
}

target "master" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  args       = { BASE_IMAGE = "base_context" }
  dockerfile = "master/Dockerfile"
  tags       = ["${REGISTRY}thavlik/dorch-master:latest"]
  push       = true
}

target "operator" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "operator/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-operator:latest"]
  push       = true
}


target "iam" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "iam/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-iam:latest"]
  push       = true
}

target "party" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "party/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-party:latest"]
  push       = true
}

target "webrtc-auth" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "webrtc-auth/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-webrtc-auth:latest"]
  push       = true
}

target "sock" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "sock/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-sock:latest"]
  push       = true
}

target "server" {
  context    = "zandronum/"
  dockerfile = "Dockerfile.server"
  tags       = ["${REGISTRY}thavlik/zandronum-server:latest"]
  push       = true
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
}

target "client" {
  context    = "zandronum/"
  dockerfile = "Dockerfile.client"
  tags       = ["${REGISTRY}thavlik/zandronum-client:latest"]
  push       = true
}
