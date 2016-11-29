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
for file in glob.glob('py/loqui/*.%s' % ext):
    package = os.path.splitext(os.path.basename(file))[0]
    print package
    extensions.append(Extension(
        'loqui.%s' % package,
        [file],
        extra_compile_args=['-O3']
    ))

if cythonize:
    extensions = cythonize(extensions)

setup(
    name='loqui',
    version='0.0.1',
    author='Jake Heinz',
    author_email='jh@discordapp.com',
    url="http://github.com/hammerandchisel/loqui",
    description='A really simple stream based RPC - with a gevent client/server implementation',
    license='MIT',
    package_dir={
        '': 'py'
    },
    packages=['loqui'],
    ext_modules=extensions,
)
