#!/usr/bin/env bash
set -e

opencv_version="4.6.0"
file_dir="tmp"
arch="$(uname -m)"

case "$arch" in
    x86_64)
        sse_opt='-DCPU_BASELINE_REQUIRE=SSE2'
    ;;

    amd64)
        sse_opt='-DCPU_BASELINE_REQUIRE=SSE2'
    ;;

    *)
        sse_opt=''
    ;;
esac

mkdir -p "${file_dir}/opencv/build"
pushd "$file_dir"

aria2c --allow-overwrite=true -x4 "https://github.com/opencv/opencv/archive/${opencv_version}.zip"
aria2c --allow-overwrite=true -x4 "https://github.com/opencv/opencv_contrib/archive/${opencv_version}.tar.gz"
bsdtar -C opencv -xvf "opencv-${opencv_version}.zip"
bsdtar -C opencv -xvf "opencv_contrib-${opencv_version}.tar.gz"

pushd opencv/build

cmake "../opencv-${opencv_version}" \
      -DWITH_OPENGL=ON \
      -DWITH_QT=ON \
      -DWITH_TBB=ON \
      -DBUILD_WITH_DEBUG_INFO=OFF \
      -DBUILD_TESTS=OFF \
      -DBUILD_PERF_TESTS=OFF \
      -DBUILD_EXAMPLES=OFF \
      -DINSTALL_C_EXAMPLES=OFF \
      -DINSTALL_PYTHON_EXAMPLES=OFF \
      "-DCMAKE_INSTALL_PREFIX=${HOME}/opt/opencv${opencv_version}" \
      -DCMAKE_ISTALL_LIBDIR=lib \
      -DCPU_BASELINE_DISABLE=SSE3 \
      ${sse_opt} \
      -DOPENCV_GENERATE_PKGCONFIG=YES \
      -DOPENCV_EXTRA_MODULES_PATH="../opencv_contrib-${opencv_version}/modules" \
      -DEIGEN_INCLUDE_PATH=`pkg-config --cflags-only-I eigen3 | sed "s/-I//"`
make -j `nproc --all`
make install

popd
popd
