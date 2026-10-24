_teak() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="teak"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        teak)
            opts="-n -L -l -A -a -x -j -h --lines --depth --long --sort --focus --series-min --detect --preview --dupes --mismatches --no-cache --templates --templates-only --dump-templates --almost-all --all --no-gitignore --chaff --show --fold --hide --one-file-system --follow --format --json-full --color --ascii --width --time --bytes --si --no-git --why --stats --threads --version --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --lines)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -n)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --depth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -L)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --sort)
                    COMPREPLY=($(compgen -W "salience name size age type" -- "${cur}"))
                    return 0
                    ;;
                --focus)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --series-min)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --detect)
                    COMPREPLY=($(compgen -W "off fast full" -- "${cur}"))
                    return 0
                    ;;
                --preview)
                    COMPREPLY=($(compgen -W "off short full" -- "${cur}"))
                    return 0
                    ;;
                --templates)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --dump-templates)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --chaff)
                    COMPREPLY=($(compgen -W "show dim fold hide" -- "${cur}"))
                    return 0
                    ;;
                --show)
                    COMPREPLY=($(compgen -W "vendor build cache vcs editor lock generated minified boilerplate empty duplicate ignored" -- "${cur}"))
                    return 0
                    ;;
                --fold)
                    COMPREPLY=($(compgen -W "vendor build cache vcs editor lock generated minified boilerplate empty duplicate ignored" -- "${cur}"))
                    return 0
                    ;;
                --hide)
                    COMPREPLY=($(compgen -W "vendor build cache vcs editor lock generated minified boilerplate empty duplicate ignored" -- "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "tty plain md json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                --width)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --time)
                    COMPREPLY=($(compgen -W "relative iso" -- "${cur}"))
                    return 0
                    ;;
                --why)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --threads)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -j)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _teak -o nosort -o bashdefault -o default teak
else
    complete -F _teak -o bashdefault -o default teak
fi
