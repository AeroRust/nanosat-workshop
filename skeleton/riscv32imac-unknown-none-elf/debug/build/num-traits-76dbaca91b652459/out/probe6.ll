; ModuleID = 'probe6.ea437712-cgu.0'
source_filename = "probe6.ea437712-cgu.0"
target datalayout = "e-m:e-p:32:32-i64:64-n32-S128"
target triple = "riscv32"

@alloc5 = private unnamed_addr constant <{ [77 x i8] }> <{ [77 x i8] c"/rustc/9aa5c24b7d763fb98d998819571128ff2eb8a3ca/library/core/src/ops/arith.rs" }>, align 1
@alloc6 = private unnamed_addr constant <{ ptr, [12 x i8] }> <{ ptr @alloc5, [12 x i8] c"M\00\00\00\01\03\00\003\00\00\00" }>, align 4
@str.0 = internal constant [28 x i8] c"attempt to add with overflow"
@alloc3 = private unnamed_addr constant <{ [4 x i8] }> <{ [4 x i8] c"\02\00\00\00" }>, align 4

; <i32 as core::ops::arith::AddAssign<&i32>>::add_assign
; Function Attrs: inlinehint nounwind
define internal void @"_ZN66_$LT$i32$u20$as$u20$core..ops..arith..AddAssign$LT$$RF$i32$GT$$GT$10add_assign17h210e9e793e75aff4E"(ptr align 4 %self, ptr align 4 %other) unnamed_addr #0 {
start:
  %other1 = load i32, ptr %other, align 4, !noundef !0
  %0 = load i32, ptr %self, align 4, !noundef !0
  %1 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %0, i32 %other1)
  %_5.0 = extractvalue { i32, i1 } %1, 0
  %_5.1 = extractvalue { i32, i1 } %1, 1
  %2 = call i1 @llvm.expect.i1(i1 %_5.1, i1 false)
  br i1 %2, label %panic, label %bb1

bb1:                                              ; preds = %start
  store i32 %_5.0, ptr %self, align 4
  ret void

panic:                                            ; preds = %start
; call core::panicking::panic
  call void @_ZN4core9panicking5panic17ha30313d16dfbd70fE(ptr align 1 @str.0, i32 28, ptr align 4 @alloc6) #5
  unreachable
}

; probe6::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe65probe17hec31590a83c7a9c4E() unnamed_addr #1 {
start:
  %x = alloca i32, align 4
  store i32 1, ptr %x, align 4
; call <i32 as core::ops::arith::AddAssign<&i32>>::add_assign
  call void @"_ZN66_$LT$i32$u20$as$u20$core..ops..arith..AddAssign$LT$$RF$i32$GT$$GT$10add_assign17h210e9e793e75aff4E"(ptr align 4 %x, ptr align 4 @alloc3) #6
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind readnone speculatable willreturn
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nofree nosync nounwind readnone willreturn
declare i1 @llvm.expect.i1(i1, i1) #3

; core::panicking::panic
; Function Attrs: cold noinline noreturn nounwind
declare dso_local void @_ZN4core9panicking5panic17ha30313d16dfbd70fE(ptr align 1, i32, ptr align 4) unnamed_addr #4

attributes #0 = { inlinehint nounwind "frame-pointer"="all" "target-cpu"="generic-rv32" "target-features"="+m,+a,+c,+a" }
attributes #1 = { nounwind "frame-pointer"="all" "target-cpu"="generic-rv32" "target-features"="+m,+a,+c,+a" }
attributes #2 = { nocallback nofree nosync nounwind readnone speculatable willreturn }
attributes #3 = { nocallback nofree nosync nounwind readnone willreturn }
attributes #4 = { cold noinline noreturn nounwind "frame-pointer"="all" "target-cpu"="generic-rv32" "target-features"="+m,+a,+c,+a" }
attributes #5 = { noreturn nounwind }
attributes #6 = { nounwind }

!0 = !{}
