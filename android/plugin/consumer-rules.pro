# btleplug's Java side, called only through JNI (its README asks for these).
-keep class com.nonpolynomial.** { *; }
-keep class io.github.gedgygedgy.** { *; }

# rustls-platform-verifier's Java side, also reached only through JNI (its README).
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
