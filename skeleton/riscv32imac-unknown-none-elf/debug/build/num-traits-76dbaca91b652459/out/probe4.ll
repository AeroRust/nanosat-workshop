; ModuleID = 'probe4.da5e3e25-cgu.0'
source_filename = "probe4.da5e3e25-cgu.0"
target datalayout = "e-m:e-p:32:32-i64:64-n32-S128"
target triple = "riscv32"

; probe4::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe45probe17h419f2cd970784f68E() unnamed_addr #0 {
start:
  %0 = alloca i32, align 4
  store i32 -2147483648, ptr %0, align 4
  %1 = load i32, ptr %0, align 4, !noundef !0
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind readnone speculatable willreturn
declare i32 @llvm.bitreverse.i32(i32) #1

attributes #0 = { nounwind "frame-pointer"="all" "target-cpu"="generic-rv32" "target-features"="+m,+a,+c,+a" }
attributes #1 = { nocallback nofree nosync nounwind readnone speculatable willreturn }

!0 = !{}
