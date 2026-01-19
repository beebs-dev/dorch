#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/"
KUBECONTEXT="${KUBECONTEXT:-do-nyc3-beeb}"
NAMESPACE="${NAMESPACE:-dorch}"
echo "Using kubectl context: $KUBECONTEXT"
do_build() {
    build_args=()
        for arg in "$@"; do
            case "$arg" in
            party-router)
                build_args+=("party")
                ;;
            *)
                build_args+=("$arg")
                ;;
        esac
    done
    ./build.sh --push "${build_args[@]}"
}
do_restart() {
    restart_args=()
    for arg in "$@"; do
        case "$arg" in
        party)
            restart_args+=("party")
            restart_args+=("party-router")
            ;;
        analyzer)
            restart_args+=("analyzer-map")
            restart_args+=("analyzer-wad")
            ;;
        server|proxy)
            restart_server=true
            kubectl delete pod --context $KUBECONTEXT -n $NAMESPACE test-game
            ;;
        *)
            restart_args+=("$arg")
            ;;
        esac
    done
    kubectl rollout restart deployment --context $KUBECONTEXT -n $NAMESPACE "${restart_args[@]/#/$NAMESPACE-}"
}
main() {
    do_build "$@"
    kubectl --context $KUBECONTEXT apply -f crds/
    do_restart "$@"
    k9s -n $NAMESPACE --splashless --context $KUBECONTEXT
}
main "$@"