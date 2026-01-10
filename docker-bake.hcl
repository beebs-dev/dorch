variable "REGISTRY" {
  default = ""
}

group "default" {
  targets = [
    "operator",
    "server"
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
  contexts   = { base_context = "target:base" }
  context    = "./"
  dockerfile = "server/Dockerfile"
  args       = { BASE_IMAGE = "base_context" }
  tags       = ["${REGISTRY}thavlik/dorch-server:latest"]
  push       = true
}
