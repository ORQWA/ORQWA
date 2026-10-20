# The Android build image (ADR 0024): the dev image plus what an APK needs, kept apart so the
# dev image, its pulls and the checks stay as small and fast as before. Built by
# scripts/android.sh on top of the local dev image, or in CI on top of the published one.
ARG BASE=torqa-dev
FROM ${BASE}

ARG GODOT_VERSION=4.7.2
# Of Godot_v4.7.2-stable_export_templates.tpz, from the release's SHA512-SUMS.txt.
ARG GODOT_TEMPLATES_SHA512=ca4d71c4d7b81dfc15d1a98baa07534aa95b03fdda78a0075b06672e1648d2e5f40980c9adc28d23e1b92e732ee7bf3461997aa804af74ec2fcd7a93ccb84079
# The SDK's command-line tools 23.0 and their SHA-1, from Google's repository2-3.xml.
ARG CMDLINE_TOOLS_BUILD=16111833
ARG CMDLINE_TOOLS_SHA1=e025545c62a8e64c7559119566a569fb1dec5f60
# What Godot 4.7.2's Android build expects (its config.gradle): SDK 36, build tools 36.1.0. The
# NDK is the newest stable one; sdkmanager verifies what it downloads.
ARG ANDROID_PLATFORM=android-36
ARG ANDROID_BUILD_TOOLS=36.1.0
ARG ANDROID_NDK=30.0.16248370

# Java for Godot's Gradle build: Godot asks for 17 or newer, and Debian 13 ships 21.
RUN apt-get update \
    && apt-get install -y --no-install-recommends openjdk-21-jdk-headless \
    && rm -rf /var/lib/apt/lists/*
ENV JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 \
    ANDROID_HOME=/opt/android-sdk
ENV ANDROID_NDK_ROOT=${ANDROID_HOME}/ndk/${ANDROID_NDK} \
    PATH=${PATH}:/opt/android-sdk/cmdline-tools/latest/bin:/opt/android-sdk/platform-tools

RUN curl -fsSL -o /tmp/tools.zip \
        "https://dl.google.com/android/repository/commandlinetools-linux-${CMDLINE_TOOLS_BUILD}_latest.zip" \
    && echo "${CMDLINE_TOOLS_SHA1}  /tmp/tools.zip" | sha1sum -c - \
    && mkdir -p "$ANDROID_HOME/cmdline-tools" \
    && unzip -q /tmp/tools.zip -d /tmp/tools \
    && mv /tmp/tools/cmdline-tools "$ANDROID_HOME/cmdline-tools/latest" \
    && rm -rf /tmp/tools /tmp/tools.zip \
    && yes | sdkmanager --licenses >/dev/null \
    && sdkmanager --install "platform-tools" "platforms;${ANDROID_PLATFORM}" \
        "build-tools;${ANDROID_BUILD_TOOLS}" "ndk;${ANDROID_NDK}" >/dev/null

RUN rustup target add aarch64-linux-android

# FFmpeg for Android, as everywhere else (ADR 0010).
COPY scripts/build-ffmpeg.sh /tmp/build-ffmpeg.sh
RUN sh /tmp/build-ffmpeg.sh /opt/ffmpeg-android android && rm /tmp/build-ffmpeg.sh

# Only the Android parts of Godot's export templates: the template APKs and the Gradle build's
# sources.
RUN curl -fsSL -o /tmp/templates.tpz \
        "https://github.com/godotengine/godot/releases/download/${GODOT_VERSION}-stable/Godot_v${GODOT_VERSION}-stable_export_templates.tpz" \
    && echo "${GODOT_TEMPLATES_SHA512}  /tmp/templates.tpz" | sha512sum -c - \
    && dir="/root/.local/share/godot/export_templates/${GODOT_VERSION}.stable" \
    && mkdir -p "$dir" \
    && unzip -q -j /tmp/templates.tpz -d "$dir" \
        templates/android_debug.apk templates/android_release.apk \
        templates/android_source.zip templates/version.txt \
    && rm /tmp/templates.tpz
