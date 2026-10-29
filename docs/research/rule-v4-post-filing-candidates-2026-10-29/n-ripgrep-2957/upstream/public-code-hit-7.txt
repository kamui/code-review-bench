#!/bin/zsh


# ---- MEZX bootstrap v1.4 START { ----
: MEZX_BOOTSTRAP ENTRY
typeset -g MEZX_BOOTSTRAP_V_filepathS=$0
setopt aliases nounset noerrexit WARN_CREATE_GLOBAL EVAL_LINENO

typeset -g -i  B_T=0  B_F=1  B_N=2
typeset -g -i  MEZX_BOOTSTRAP_V_errB=${B_F}
typeset -g     MEZX_BOOTSTRAP_V_errS=


function MEZX_BOOTSTRAP_F_main() {
    typeset +g     filenameS="${MEZX_BOOTSTRAP_V_filepathS:t}_$$"
    typeset +g     errS  sepS  argL_S  pathnameS  pathcacheS  tmppathS="/tmp/${filenameS}"
    typeset +g -i  resultI  errI=0  errB=${B_T}  argcI=$#
    typeset +g -a  argL=( "${(@)@}" )
    
    while true; do
        if [[ ${+MEZXDIR} -eq 0 ]]; then
            errI=1;  errS="MEZXDIR var not defined";  break;  fi
        if [[ ${+MEZXDIR_STATUS} -eq 1 ]]; then
            if [[ ${MEZXDIR_STATUS} = loaded ]]; then  errB=${B_F};  break;  fi
            if [[ ${MEZXDIR_STATUS} = loading ]]; then  errB=${B_F};  break;  fi
            if [[ ${MEZXDIR_STATUS} = failed ]]; then
                errI=2;  print -v errS -f "MEZX framework inclusion previously failed; MEZXDIR_STATUS=failed";  break;  fi;  fi
        typeset -g MEZXDIR_STATUS=loading
        pathnameS="${MEZXDIR}/internal/framework.zsh"
        pathcacheS="${MEZXDIR}/cache/INTERNAL_FRAMEWORK.cache.zsh"
	if ! [[ -f ${pathnameS} ]]; then
	    typeset -g MEZXDIR_STATUS=failed
            print -v errS -f "MEZX framework file does not exist: MEZXDIR=%s;  pathnameS=%s" -- \
                ${MEZXDIR}  ${pathnameS}
            errI=3;  break;  fi
	if ! [[ -f ${pathcacheS} ]]; then
	    typeset -g MEZXDIR_STATUS=failed
	    print -v errS \
		-f "MEZX cached framework file does not exist: MEZXDIR=%s;  pathcacheS=%s" -- \
		${MEZXDIR}  ${pathcacheS}
	    errI=4;  break
	elif ! [[ ${pathnameS} -ot ${pathcacheS} ]]; then
	    print -v errS \
		-f "MEZX cached framework file is out of date: MEZXDIR=%s;  pathcacheS=%s;  pathnameS=%s" -- \
		${MEZXDIR}  ${pathcacheS}  ${pathnameS}
	    errI=5;  break
	else
	    . ${pathcacheS}  >${tmppathS}_out  2>${tmppathS}_err;  resultI=$?
	    if [[ ${resultI} -ne ${B_T} ]]; then
		typeset -g MEZXDIR_STATUS=failed
		print -v errS \
		    -f "MEZX cached framework file inclusion failed: MEZXDIR=%s;  pathcacheS=%s;  resultI=%d;  outS=%s;  errS=%s" -- \
		    ${MEZXDIR}  ${pathcacheS}  ${resultI} \
		    "$(cat ${tmppathS}_out)"  "$(cat ${tmppathS}_err)"
		errI=6;  break;  fi;  fi
	typeset -g MEZXDIR_STATUS=loaded;  errB=${B_F}
	break;  done
    rm -f ${tmppathS}_out ${tmppathS}_err
    if [[ ${errB} -eq ${B_F} ]]; then
        typeset -g -i  MEZX_BOOTSTRAP_V_errB=${B_F};  fi
    if [[ ${errB} -ne ${B_F} ]]; then
        typeset -g -i  MEZX_BOOTSTRAP_V_errB=${B_T}
        typeset -g -i  MEZX_BOOTSTRAP_V_errI=${errI}
	sepS=  argL_S=
	if [[ ${argcI} -gt 0 ]]; then
	    for indexI in {1..${argcI}}; do
		print -v argL_S -f "%s%s[%d]%s" --  "${argL_S}"  "${sepS}"  ${indexI}  ${argL[${indexI}]};  sepS="  "; done;  fi
        print -v errS -f "%s:  { %s;  errI=%d };  argL=( %s )" --  $0  ${errS}  ${errI}  ${argL_S};
        typeset -g     MEZX_BOOTSTRAP_V_errS=${errS};  fi;
    if [[ ${errB} -eq ${B_F} ]]; then
	: MEZX_BOOTSTRAP_F_main EXIT \(ok\)
	return ${B_T}; fi
    : MEZX_BOOTSTRAP_F_main EXIT \(error\)
    return ${B_F};
}


MEZX_BOOTSTRAP_F_main;  typeset +g -i resultI=$?
if [[ ${resultI} -ne ${B_T} ]]; then
    print -f "ERROR: %s:  { %s;  errI=%d }\n" --  $0  ${MEZX_BOOTSTRAP_V_errS}  ${MEZX_BOOTSTRAP_V_errI}
    return ${B_F};  fi
return ${B_T}
# ---- MEZX bootstrap v1.4 END } ----
