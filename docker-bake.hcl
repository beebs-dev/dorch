variable "REGISTRY" {
  default = ""
}

group "default" {
  targets = [
    "operator",
    "server",
    "proxy",
    "client",
    "downloader"
  ]
}

target "base" {
  context    = "./"
  dockerfile = "Dockerfile.base"
  tags       = ["${REGISTRY}thavlik/dorch-base:latest"]
  push       = false
}

target "operator" {
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "operator/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-operator:latest"]
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
