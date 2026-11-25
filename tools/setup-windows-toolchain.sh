#!/bin/sh
set -e
t=/var/www/swirl/.toolchain
mkdir -p $t/bin
python3 -m pip install --quiet --target $t/zigpy ziglang
z=$t/zigpy/ziglang/zig
w=$(ls -d $HOME/.cargo/registry/src/*/winapi-x86_64-pc-windows-gnu-0.4.0/lib | head -1)
cat > $t/unwind_stub.c <<'C'
#include <stdlib.h>
#define STUB(n) void n(void) { abort(); }
STUB(_GCC_specific_handler)
STUB(_Unwind_GetDataRelBase)
STUB(_Unwind_GetIPInfo)
STUB(_Unwind_GetLanguageSpecificData)
STUB(_Unwind_GetRegionStart)
STUB(_Unwind_GetTextRelBase)
STUB(_Unwind_Resume)
STUB(_Unwind_SetGR)
STUB(_Unwind_SetIP)
STUB(_Unwind_RaiseException)
STUB(_Unwind_DeleteException)
STUB(_Unwind_Backtrace)
STUB(_Unwind_GetCFA)
STUB(_Unwind_GetIP)
C
$z cc -target x86_64-windows-gnu -c $t/unwind_stub.c -o $t/unwind_stub.o
cat > $t/zcc-win.sh <<EOF2
#!/bin/sh
extra=$t/unwind_stub.o
for a in "\$@"; do shift; case "\$a" in -c|-E|-S) extra=;; esac; case "\$a" in --target=*|-lgcc_eh|-lgcc|-lmsvcrt|-lpthread|-l:libpthread.a) ;; -lsynchronization) set -- "\$@" -lapi-ms-win-core-synch-l1-2-0;; -lwinapi_*) set -- "\$@" "$w/lib\${a#-l}.a";; *) set -- "\$@" "\$a";; esac; done
exec $z cc -target x86_64-windows-gnu "\$@" \$extra
EOF2
printf '#!/bin/sh\nexec %s ar "$@"\n' $z > $t/zar.sh
printf '#!/bin/sh\nexec %s dlltool "$@"\n' $z > $t/bin/x86_64-w64-mingw32-dlltool
chmod +x $t/zcc-win.sh $t/zar.sh $t/bin/*
cat > $t/env.sh <<EOF2
export PATH=$t/bin:\$HOME/.cargo/bin:\$PATH
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$t/zcc-win.sh CC_x86_64_pc_windows_gnu=$t/zcc-win.sh AR_x86_64_pc_windows_gnu=$t/zar.sh
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-self-contained=no"
EOF2
echo done
