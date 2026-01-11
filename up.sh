#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/"
KUBECONTEXT="${KUBECONTEXT:-do-nyc1-beebs}"
NAMESPACE="${NAMESPACE:-dorch}"
echo "Using kubectl context: $KUBECONTEXT"
do_build() {
    build_args=()
        for arg in "$@"; do
            case "$arg" in
            *)
                build_args+=("$arg")
                ;;
        esac
    done
    ./build.sh --push "${build_args[@]}"
}
do_restart() {
    restart_args=()
    restart_app=false
    for arg in "$@"; do
        case "$arg" in
        client)
            restart_app=true
            ;;
        *)
            restart_args+=("$arg")
            ;;
        esac
    done
    if [ "$restart_app" = true ] || [ "${#restart_args[@]}" -eq 0 ]; then
        kubectl rollout restart deployment --context $KUBECONTEXT -n apps apps-prboom
    fi
    kubectl rollout restart deployment --context $KUBECONTEXT -n $NAMESPACE "${restart_args[@]/#/$NAMESPACE-}"
}
main() {
    do_build "$@"
    kubectl --context $KUBECONTEXT apply -f crds/
    do_restart "$@"
    k9s -n $NAMESPACE --splashless --context $KUBECONTEXT
}
main "$@"