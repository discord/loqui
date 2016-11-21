import os
from distutils.core import setup
from distutils.extension import Extension
import glob

try:
    from Cython.Build import cythonize
    ext = 'pyx'
except ImportError:
    cythonize = None
    ext = 'c'


extensions = []
for file in glob.glob('py/drpc/*.%s' % ext):
    package = os.path.splitext(os.path.basename(file))[0]
    print package
    extensions.append(Extension(
        'drpc.%s' % package,
        [file],
        extra_compile_args=['-O3']
    ))

if cythonize:
    extensions = cythonize(extensions)

setup(
    name='drpc',
    version='0.0.1',
    author='Jake Heinz',
    author_email='jh@discordapp.com',
    url="http://github.com/hammerandchisel/drpc",
    description='A really simple stream based RPC - with a gevent client/server implementation',
    license='MIT',
    package_dir={
        'drpc': 'py/drpc'
    },
    ext_modules=extensions,
)
