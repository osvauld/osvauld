(function(Object){
   typeof globalThis !== "object"
   &&
    (this
      ? get()
      : (Object.defineProperty
         (Object.prototype, "_T_", {configurable: true, get: get}),
        _T_));
   function get(){
    var global = this || self;
    global.globalThis = global;
    delete Object.prototype._T_;
   }
  }
  (Object));
(js=>
     async args=>{
      "use strict";
      const
       {link, src, generated, disable_effects} = args,
       isNode = globalThis.process?.versions?.node,
       math =
         {cos: Math.cos,
          sin: Math.sin,
          tan: Math.tan,
          acos: Math.acos,
          asin: Math.asin,
          atan: Math.atan,
          cosh: Math.cosh,
          sinh: Math.sinh,
          tanh: Math.tanh,
          acosh: Math.acosh,
          asinh: Math.asinh,
          atanh: Math.atanh,
          cbrt: Math.cbrt,
          exp: Math.exp,
          expm1: Math.expm1,
          log: Math.log,
          log1p: Math.log1p,
          log2: Math.log2,
          log10: Math.log10,
          atan2: Math.atan2,
          hypot: Math.hypot,
          pow: Math.pow,
          fmod: (x, y)=>x % y},
       typed_arrays =
         [Float32Array,
          Float64Array,
          Int8Array,
          Uint8Array,
          Int16Array,
          Uint16Array,
          Int32Array,
          Int32Array,
          Int32Array,
          Int32Array,
          Float32Array,
          Float64Array,
          Uint8Array,
          Uint16Array,
          Uint8ClampedArray],
       fs = isNode && require("node:fs"),
       fs_cst = fs?.constants,
       access_flags =
         fs ? [fs_cst.R_OK, fs_cst.W_OK, fs_cst.X_OK, fs_cst.F_OK] : [],
       open_flags =
         fs
          ? [fs_cst.O_RDONLY,
            fs_cst.O_WRONLY,
            fs_cst.O_RDWR,
            fs_cst.O_APPEND,
            fs_cst.O_CREAT,
            fs_cst.O_TRUNC,
            fs_cst.O_EXCL,
            fs_cst.O_NONBLOCK,
            fs_cst.O_NOCTTY,
            fs_cst.O_DSYNC,
            fs_cst.O_SYNC]
          : [];
      var
       out_channels =
         {map: new WeakMap(),
          set: new Set(),
          finalization:
          new FinalizationRegistry(ref=>out_channels.set.delete(ref))};
      function register_channel(ch){
       const ref = new WeakRef(ch);
       out_channels.map.set(ch, ref);
       out_channels.set.add(ref);
       out_channels.finalization.register(ch, ref, ch);
      }
      function unregister_channel(ch){
       const ref = out_channels.map.get(ch);
       if(ref){
        out_channels.map.delete(ch);
        out_channels.set.delete(ref);
        out_channels.finalization.unregister(ch);
       }
      }
      function channel_list(){
       return [...out_channels.set].map(ref=>ref.deref()).filter(ch=>ch);
      }
      var start_fiber;
      function make_suspending(f){
       return WebAssembly?.Suspending ? new WebAssembly.Suspending(f) : f;
      }
      function make_promising(f){
       return ! disable_effects && WebAssembly?.promising && f
               ? WebAssembly.promising(f)
               : f;
      }
      const
       decoder = new TextDecoder("utf-8", {ignoreBOM: 1}),
       encoder = new TextEncoder();
      function hash_int(h, d){
       d = Math.imul(d, 0xcc9e2d51 | 0);
       d = d << 15 | d >>> 17;
       d = Math.imul(d, 0x1b873593);
       h ^= d;
       h = h << 13 | h >>> 19;
       return (h + (h << 2) | 0) + (0xe6546b64 | 0) | 0;
      }
      function hash_string(h, s){
       for(var i = 0; i < s.length; i++) h = hash_int(h, s.charCodeAt(i));
       return h ^ s.length;
      }
      function getenv(n){
       if(isNode && globalThis.process.env[n] !== undefined)
        return globalThis.process.env[n];
       return globalThis.jsoo_env?.[n];
      }
      let record_backtrace_flag = 0;
      for(const l of getenv("OCAMLRUNPARAM")?.split(",") || []){
       if(l === "b") record_backtrace_flag = 1;
       if(l.startsWith("b=")) record_backtrace_flag = + l.slice(2) ? 1 : 0;
      }
      function alloc_stat(s, large){
       var kind;
       if(s.isFile())
        kind = 0;
       else if(s.isDirectory())
        kind = 1;
       else if(s.isCharacterDevice())
        kind = 2;
       else if(s.isBlockDevice())
        kind = 3;
       else if(s.isSymbolicLink())
        kind = 4;
       else if(s.isFIFO()) kind = 5; else if(s.isSocket()) kind = 6;
       return caml_alloc_stat
               (large,
                s.dev,
                s.ino | 0,
                kind,
                s.mode,
                s.nlink,
                s.uid,
                s.gid,
                s.rdev,
                BigInt(s.size),
                s.atimeMs / 1000,
                s.mtimeMs / 1000,
                s.ctimeMs / 1000);
      }
      const
       on_windows = isNode && globalThis.process.platform === "win32",
       call = Function.prototype.call,
       DV = DataView.prototype,
       bindings =
         {jstag:
          WebAssembly.JSTag
          || new WebAssembly.Tag({parameters: ["externref"], results: []}),
          identity: x=>x,
          from_bool: x=>! ! x,
          get: (x, y)=>x[y],
          set: (x, y, z)=>x[y] = z,
          delete: (x, y)=>delete x[y],
          instanceof: (x, y)=>x instanceof y,
          typeof: x=>typeof x,
          equals: (x, y)=>x == y,
          strict_equals: (x, y)=>x === y,
          fun_call: (f, o, args)=>f.apply(o, args),
          meth_call: (o, f, args)=>o[f].apply(o, args),
          new_array: n=>new Array(n),
          new_obj: ()=>({}),
          new: (c, args)=>new c(...args),
          global_this: globalThis,
          iter_props:
          (o, f)=>{for(var nm in o) if(Object.hasOwn(o, nm)) f(nm);},
          array_length: a=>a.length,
          array_get: (a, i)=>a[i],
          array_set: (a, i, v)=>a[i] = v,
          read_string: l=>decoder.decode(new Uint8Array(buffer, 0, l)),
          read_string_stream:
          (l, stream)=>
             decoder.decode(new Uint8Array(buffer, 0, l), {stream: stream}),
          append_string: (s1, s2)=>s1 + s2,
          write_string:
          s=>{
           var start = 0, len = s.length;
           for(;;){
            const
             {read, written} = encoder.encodeInto(s.slice(start), out_buffer);
            len -= read;
            if(! len) return written;
            caml_extract_bytes(written);
            start += read;
           }},
          ta_create: (k, sz)=>new typed_arrays[k](sz),
          ta_normalize:
          a=>
             a instanceof Uint32Array
              ? new Int32Array(a.buffer, a.byteOffset, a.length)
              : a,
          ta_kind: a=>typed_arrays.findIndex(c=>a instanceof c),
          ta_length: a=>a.length,
          ta_get_i32: (a, i)=>a[i],
          ta_fill: (a, v)=>a.fill(v),
          ta_blit: (s, d)=>d.set(s),
          ta_subarray: (a, i, j)=>a.subarray(i, j),
          ta_set: (a, b, i)=>a.set(b, i),
          ta_new: len=>new Uint8Array(len),
          ta_copy: (ta, t, s, e)=>ta.copyWithin(t, s, e),
          ta_bytes:
          a=>
             new
              Uint8Array
              (a.buffer, a.byteOffset, a.length * a.BYTES_PER_ELEMENT),
          ta_blit_from_bytes:
          (s, p1, a, p2, l)=>{
           for(let i = 0; i < l; i++) a[p2 + i] = bytes_get(s, p1 + i);},
          ta_blit_to_bytes:
          (a, p1, s, p2, l)=>{
           for(let i = 0; i < l; i++) bytes_set(s, p2 + i, a[p1 + i]);},
          dv_make: a=>new DataView(a.buffer, a.byteOffset, a.byteLength),
          dv_get_f64: call.bind(DV.getFloat64),
          dv_get_f32: call.bind(DV.getFloat32),
          dv_get_i64: call.bind(DV.getBigInt64),
          dv_get_i32: call.bind(DV.getInt32),
          dv_get_i16: call.bind(DV.getInt16),
          dv_get_ui16: call.bind(DV.getUint16),
          dv_get_i8: call.bind(DV.getInt8),
          dv_get_ui8: call.bind(DV.getUint8),
          dv_set_f64: call.bind(DV.setFloat64),
          dv_set_f32: call.bind(DV.setFloat32),
          dv_set_i64: call.bind(DV.setBigInt64),
          dv_set_i32: call.bind(DV.setInt32),
          dv_set_i16: call.bind(DV.setInt16),
          dv_set_i8: call.bind(DV.setInt8),
          littleEndian: new Uint8Array(new Uint32Array([1]).buffer)[0],
          wrap_callback:
          f=>
             function(...args){
              if(args.length === 0) args = [undefined];
              return caml_callback(f, args.length, args, 1);
             },
          wrap_callback_args:
          f=>function(...args){return caml_callback(f, 1, [args], 0);},
          wrap_callback_strict:
          (arity, f)=>
             function(...args){
              args.length = arity;
              return caml_callback(f, arity, args, 0);
             },
          wrap_callback_unsafe:
          f=>function(...args){return caml_callback(f, args.length, args, 2);},
          wrap_meth_callback:
          f=>
             function(...args){
              args.unshift(this);
              return caml_callback(f, args.length, args, 1);
             },
          wrap_meth_callback_args:
          f=>function(...args){return caml_callback(f, 2, [this, args], 0);},
          wrap_meth_callback_strict:
          (arity, f)=>
             function(...args){
              args.length = arity;
              args.unshift(this);
              return caml_callback(f, args.length, args, 0);
             },
          wrap_meth_callback_unsafe:
          f=>
             function(...args){
              args.unshift(this);
              return caml_callback(f, args.length, args, 2);
             },
          wrap_fun_arguments: f=>function(...args){return f(args);},
          format_float:
          (prec, conversion, pad, x)=>{
           function toFixed(x, dp){
            if(Math.abs(x) < 1.0)
             return x.toFixed(dp);
            else{
             var e = Number.parseInt(x.toString().split("+")[1]);
             if(e > 20){
              e -= 20;
              x /= Math.pow(10, e);
              x += new Array(e + 1).join("0");
              if(dp > 0) x = x + "." + new Array(dp + 1).join("0");
              return x;
             }
             else
              return x.toFixed(dp);
            }
           }
           switch(conversion){
             case 0:
              var s = x.toExponential(prec), i = s.length;
              if(s.charAt(i - 3) === "e")
               s = s.slice(0, i - 1) + "0" + s.slice(i - 1);
              break;
             case 1:
              s = toFixed(x, prec); break;
             case 2:
              prec = prec ? prec : 1;
              s = x.toExponential(prec - 1);
              var j = s.indexOf("e"), exp = + s.slice(j + 1);
              if(exp < - 4 || x >= 1e21 || x.toFixed(0).length > prec){
               var i = j - 1;
               while(s.charAt(i) === "0") i--;
               if(s.charAt(i) === ".") i--;
               s = s.slice(0, i + 1) + s.slice(j);
               i = s.length;
               if(s.charAt(i - 3) === "e")
                s = s.slice(0, i - 1) + "0" + s.slice(i - 1);
               break;
              }
              else{
               var p = prec;
               if(exp < 0){
                p -= exp + 1;
                s = x.toFixed(p);
               }
               else
                while(s = x.toFixed(p), s.length > prec + 1) p--;
               if(p){
                var i = s.length - 1;
                while(s.charAt(i) === "0") i--;
                if(s.charAt(i) === ".") i--;
                s = s.slice(0, i + 1);
               }
              }
              break;
           }
           return pad ? " " + s : s;},
          gettimeofday: ()=>new Date().getTime() / 1000,
          times:
          ()=>{
           if(globalThis.process?.cpuUsage){
            var t = globalThis.process.cpuUsage();
            return caml_alloc_times(t.user / 1e6, t.system / 1e6);
           }
           else{
            var t = performance.now() / 1000;
            return caml_alloc_times(t, t);
           }},
          gmtime:
          t=>{
           var
            d = new Date(t * 1000),
            d_num = d.getTime(),
            januaryfirst =
              new Date(Date.UTC(d.getUTCFullYear(), 0, 1)).getTime(),
            doy = Math.floor((d_num - januaryfirst) / 86400000);
           return caml_alloc_tm
                   (d.getUTCSeconds(),
                    d.getUTCMinutes(),
                    d.getUTCHours(),
                    d.getUTCDate(),
                    d.getUTCMonth(),
                    d.getUTCFullYear() - 1900,
                    d.getUTCDay(),
                    doy,
                    false);},
          localtime:
          t=>{
           var
            d = new Date(t * 1000),
            d_num = d.getTime(),
            januaryfirst = new Date(d.getFullYear(), 0, 1).getTime(),
            doy = Math.floor((d_num - januaryfirst) / 86400000),
            jan = new Date(d.getFullYear(), 0, 1),
            jul = new Date(d.getFullYear(), 6, 1),
            stdTimezoneOffset =
              Math.max(jan.getTimezoneOffset(), jul.getTimezoneOffset());
           return caml_alloc_tm
                   (d.getSeconds(),
                    d.getMinutes(),
                    d.getHours(),
                    d.getDate(),
                    d.getMonth(),
                    d.getFullYear() - 1900,
                    d.getDay(),
                    doy,
                    d.getTimezoneOffset() < stdTimezoneOffset);},
          mktime:
          (year, month, day, h, m, s)=>
             new Date(year, month, day, h, m, s).getTime(),
          random_seed: ()=>crypto.getRandomValues(new Int32Array(12)),
          access:
          (p, flags)=>
             fs.accessSync
              (p,
               access_flags.reduce((f, v, i)=>flags & 1 << i ? f | v : f, 0)),
          open:
          (p, flags, perm)=>
             fs.openSync
              (p,
               open_flags.reduce((f, v, i)=>flags & 1 << i ? f | v : f, 0),
               perm),
          close: fd=>fs.closeSync(fd),
          write:
          (fd, b, o, l, p)=>
             fs
              ? fs.writeSync(fd, b, o, l, p === null ? p : Number(p))
              : (console
                  [fd === 2 ? "error" : "log"]
                 (typeof b === "string"
                   ? b
                   : decoder.decode(b.slice(o, o + l))),
                l),
          read: (fd, b, o, l, p)=>fs.readSync(fd, b, o, l, p),
          fsync: fd=>fs.fsyncSync(fd),
          file_size: fd=>fs.fstatSync(fd, {bigint: true}).size,
          register_channel: register_channel,
          unregister_channel: unregister_channel,
          channel_list: channel_list,
          exit: n=>isNode && globalThis.process.exit(n),
          argv: ()=>isNode ? globalThis.process.argv.slice(1) : ["a.out"],
          on_windows: + on_windows,
          getenv: getenv,
          backtrace_status: ()=>record_backtrace_flag,
          record_backtrace: b=>record_backtrace_flag = b,
          system:
          c=>{
           var
            res =
              require("node:child_process").spawnSync
               (c, {shell: true, stdio: "inherit"});
           if(res.error) throw res.error;
           return res.signal ? 255 : res.status;},
          isatty: fd=>+ require("node:tty").isatty(fd),
          time: ()=>performance.now(),
          getcwd: ()=>isNode ? globalThis.process.cwd() : "/static",
          chdir: x=>globalThis.process.chdir(x),
          mkdir: (p, m)=>fs.mkdirSync(p, m),
          rmdir: p=>fs.rmdirSync(p),
          link: (d, s)=>fs.linkSync(d, s),
          symlink:
          (t, p, kind)=>fs.symlinkSync(t, p, [null, "file", "dir"][kind]),
          readlink: p=>fs.readlinkSync(p),
          unlink: p=>fs.unlinkSync(p),
          read_dir: p=>fs.readdirSync(p),
          opendir: p=>fs.opendirSync(p),
          readdir:
          d=>{var n = d.readSync()?.name; return n === undefined ? null : n;},
          closedir: d=>d.closeSync(),
          stat: (p, l)=>alloc_stat(fs.statSync(p), l),
          lstat: (p, l)=>alloc_stat(fs.lstatSync(p), l),
          fstat: (fd, l)=>alloc_stat(fs.fstatSync(fd), l),
          chmod: (p, perms)=>fs.chmodSync(p, perms),
          fchmod: (p, perms)=>fs.fchmodSync(p, perms),
          file_exists: p=>+ fs.existsSync(p),
          is_directory: p=>+ fs.lstatSync(p).isDirectory(),
          is_file: p=>+ fs.lstatSync(p).isFile(),
          utimes: (p, a, m)=>fs.utimesSync(p, a, m),
          truncate: (p, l)=>fs.truncateSync(p, l),
          ftruncate: (fd, l)=>fs.ftruncateSync(fd, l),
          rename:
          (o, n)=>{
           var n_stat;
           if
            (on_windows && (n_stat = fs.statSync(n, {throwIfNoEntry: false}))
             && fs.statSync(o, {throwIfNoEntry: false})?.isDirectory())
            if(n_stat.isDirectory()){
             if(! n.startsWith(o)) try{fs.rmdirSync(n);}catch{}
            }
            else{
             var
              e =
                new Error(`ENOTDIR: not a directory, rename '${o}' -> '${n}'`);
             throw Object.assign
                    (e,
                     {errno: - 20, code: "ENOTDIR", syscall: "rename", path: n});
            }
           fs.renameSync(o, n);},
          tmpdir: ()=>require("node:os").tmpdir(),
          start_fiber: x=>start_fiber(x),
          suspend_fiber: make_suspending((f, env)=>new Promise(k=>f(k, env))),
          resume_fiber: (k, v)=>k(v),
          weak_new: v=>new WeakRef(v),
          weak_deref:
          w=>{var v = w.deref(); return v === undefined ? null : v;},
          weak_map_new: ()=>new WeakMap(),
          map_new: ()=>new Map(),
          map_get:
          (m, x)=>{var v = m.get(x); return v === undefined ? null : v;},
          map_set: (m, x, v)=>m.set(x, v),
          map_delete: (m, x)=>m.delete(x),
          hash_string: hash_string,
          log: x=>console.log(x)},
       string_ops =
         {test: v=>+ (typeof v === "string"),
          compare: (s1, s2)=>s1 < s2 ? - 1 : + (s1 > s2),
          decodeStringFromUTF8Array: ()=>"",
          encodeStringToUTF8Array: ()=>0,
          fromCharCodeArray: ()=>""},
       imports =
         Object.assign
          ({Math: math,
            bindings: bindings,
            js: js,
            "wasm:js-string": string_ops,
            "wasm:text-decoder": string_ops,
            "wasm:text-encoder": string_ops,
            str: new globalThis.Proxy({}, {get(_, prop){return prop;}}),
            env: {}},
           generated),
       options =
         {builtins: ["js-string", "text-decoder", "text-encoder"],
          importedStringConstants: "str"};
      function loadRelative(src){
       const
        path = require("node:path"),
        f = path.join(path.dirname(require.main.filename), src);
       return require("node:fs/promises").readFile(f);
      }
      const fetchBase = globalThis?.document?.currentScript?.src;
      function fetchRelative(src){
       const url = fetchBase ? new URL(src, fetchBase) : src;
       return fetch(url);
      }
      const loadCode = isNode ? loadRelative : fetchRelative;
      async function instantiateModule(code){
       return isNode
               ? WebAssembly.instantiate(await code, imports, options)
               : WebAssembly.instantiateStreaming(code, imports, options);
      }
      async function instantiateFromDir(){
       imports.OCaml = {};
       const deps = [];
       async function loadModule(module, isRuntime){
        const sync = module[1].constructor !== Array;
        async function instantiate(){
         const code = loadCode(src + "/" + module[0] + ".wasm");
         await Promise.all(sync ? deps : module[1].map(i=>deps[i]));
         const wasmModule = await instantiateModule(code);
         Object.assign
          (isRuntime ? imports.env : imports.OCaml,
           wasmModule.instance.exports);
        }
        const promise = instantiate();
        deps.push(promise);
        return promise;
       }
       async function loadModules(lst){
        for(const module of lst) await loadModule(module);
       }
       await loadModule(link[0], 1);
       if(link.length > 1){
        await loadModule(link[1]);
        const
         workers = new Array(20).fill(link.slice(2).values()).map(loadModules);
        await Promise.all(workers);
       }
       return {instance: {exports: Object.assign(imports.env, imports.OCaml)}};
      }
      const wasmModule = await instantiateFromDir();
      var
       {caml_callback,
         caml_alloc_times,
         caml_alloc_tm,
         caml_alloc_stat,
         caml_start_fiber,
         caml_handle_uncaught_exception,
         caml_buffer,
         caml_extract_bytes,
         bytes_get,
         bytes_set,
         _initialize}
        = wasmModule.instance.exports,
       buffer = caml_buffer?.buffer,
       out_buffer = buffer && new Uint8Array(buffer, 0, buffer.length);
      start_fiber = make_promising(caml_start_fiber);
      var _initialize = make_promising(_initialize);
      if(globalThis.process?.on)
       globalThis.process.on
        ("uncaughtException",
         (err, _origin)=>caml_handle_uncaught_exception(err));
      else if(globalThis.addEventListener)
       globalThis.addEventListener
        ("error",
         event=>event.error && caml_handle_uncaught_exception(event.error));
      await _initialize();})
 (function(globalThis){
    "use strict";
    var
     blake2b =
       function(){
         function ADD64AA(v, a, b){
          const o0 = v[a] + v[b];
          let o1 = v[a + 1] + v[b + 1];
          if(o0 >= 0x100000000) o1++;
          v[a] = o0;
          v[a + 1] = o1;
         }
         function ADD64AC(v, a, b0, b1){
          let o0 = v[a] + b0;
          if(b0 < 0) o0 += 0x100000000;
          let o1 = v[a + 1] + b1;
          if(o0 >= 0x100000000) o1++;
          v[a] = o0;
          v[a + 1] = o1;
         }
         function B2B_GET32(arr, i){
          return arr[i] ^ arr[i + 1] << 8 ^ arr[i + 2] << 16
                 ^ arr[i + 3] << 24;
         }
         function B2B_G(a, b, c, d, ix, iy){
          const x0 = m[ix], x1 = m[ix + 1], y0 = m[iy], y1 = m[iy + 1];
          ADD64AA(v, a, b);
          ADD64AC(v, a, x0, x1);
          let xor0 = v[d] ^ v[a], xor1 = v[d + 1] ^ v[a + 1];
          v[d] = xor1;
          v[d + 1] = xor0;
          ADD64AA(v, c, d);
          xor0 = v[b] ^ v[c];
          xor1 = v[b + 1] ^ v[c + 1];
          v[b] = xor0 >>> 24 ^ xor1 << 8;
          v[b + 1] = xor1 >>> 24 ^ xor0 << 8;
          ADD64AA(v, a, b);
          ADD64AC(v, a, y0, y1);
          xor0 = v[d] ^ v[a];
          xor1 = v[d + 1] ^ v[a + 1];
          v[d] = xor0 >>> 16 ^ xor1 << 16;
          v[d + 1] = xor1 >>> 16 ^ xor0 << 16;
          ADD64AA(v, c, d);
          xor0 = v[b] ^ v[c];
          xor1 = v[b + 1] ^ v[c + 1];
          v[b] = xor1 >>> 31 ^ xor0 << 1;
          v[b + 1] = xor0 >>> 31 ^ xor1 << 1;
         }
         const
          BLAKE2B_IV32 =
            new
             Uint32Array
             ([0xf3bcc908,
               0x6a09e667,
               0x84caa73b,
               0xbb67ae85,
               0xfe94f82b,
               0x3c6ef372,
               0x5f1d36f1,
               0xa54ff53a,
               0xade682d1,
               0x510e527f,
               0x2b3e6c1f,
               0x9b05688c,
               0xfb41bd6b,
               0x1f83d9ab,
               0x137e2179,
               0x5be0cd19]),
          SIGMA8 =
            [0,
             1,
             2,
             3,
             4,
             5,
             6,
             7,
             8,
             9,
             10,
             11,
             12,
             13,
             14,
             15,
             14,
             10,
             4,
             8,
             9,
             15,
             13,
             6,
             1,
             12,
             0,
             2,
             11,
             7,
             5,
             3,
             11,
             8,
             12,
             0,
             5,
             2,
             15,
             13,
             10,
             14,
             3,
             6,
             7,
             1,
             9,
             4,
             7,
             9,
             3,
             1,
             13,
             12,
             11,
             14,
             2,
             6,
             5,
             10,
             4,
             0,
             15,
             8,
             9,
             0,
             5,
             7,
             2,
             4,
             10,
             15,
             14,
             1,
             11,
             12,
             6,
             8,
             3,
             13,
             2,
             12,
             6,
             10,
             0,
             11,
             8,
             3,
             4,
             13,
             7,
             5,
             15,
             14,
             1,
             9,
             12,
             5,
             1,
             15,
             14,
             13,
             4,
             10,
             0,
             7,
             6,
             3,
             9,
             2,
             8,
             11,
             13,
             11,
             7,
             14,
             12,
             1,
             3,
             9,
             5,
             0,
             15,
             4,
             8,
             6,
             2,
             10,
             6,
             15,
             14,
             9,
             11,
             3,
             0,
             8,
             12,
             2,
             13,
             7,
             1,
             4,
             10,
             5,
             10,
             2,
             8,
             4,
             7,
             6,
             1,
             5,
             15,
             11,
             9,
             14,
             3,
             12,
             13,
             0,
             0,
             1,
             2,
             3,
             4,
             5,
             6,
             7,
             8,
             9,
             10,
             11,
             12,
             13,
             14,
             15,
             14,
             10,
             4,
             8,
             9,
             15,
             13,
             6,
             1,
             12,
             0,
             2,
             11,
             7,
             5,
             3],
          SIGMA82 = new Uint8Array(SIGMA8.map(function(x){return x * 2;})),
          v = new Uint32Array(32),
          m = new Uint32Array(32);
         function blake2bCompress(ctx, last){
          let i = 0;
          for(i = 0; i < 16; i++){
           v[i] = ctx.h[i];
           v[i + 16] = BLAKE2B_IV32[i];
          }
          v[24] = v[24] ^ ctx.t;
          v[25] = v[25] ^ ctx.t / 0x100000000;
          if(last){v[28] = ~ v[28]; v[29] = ~ v[29];}
          for(i = 0; i < 32; i++) m[i] = B2B_GET32(ctx.b, 4 * i);
          for(i = 0; i < 12; i++){
           B2B_G(0, 8, 16, 24, SIGMA82[i * 16 + 0], SIGMA82[i * 16 + 1]);
           B2B_G(2, 10, 18, 26, SIGMA82[i * 16 + 2], SIGMA82[i * 16 + 3]);
           B2B_G(4, 12, 20, 28, SIGMA82[i * 16 + 4], SIGMA82[i * 16 + 5]);
           B2B_G(6, 14, 22, 30, SIGMA82[i * 16 + 6], SIGMA82[i * 16 + 7]);
           B2B_G(0, 10, 20, 30, SIGMA82[i * 16 + 8], SIGMA82[i * 16 + 9]);
           B2B_G(2, 12, 22, 24, SIGMA82[i * 16 + 10], SIGMA82[i * 16 + 11]);
           B2B_G(4, 14, 16, 26, SIGMA82[i * 16 + 12], SIGMA82[i * 16 + 13]);
           B2B_G(6, 8, 18, 28, SIGMA82[i * 16 + 14], SIGMA82[i * 16 + 15]);
          }
          for(i = 0; i < 16; i++) ctx.h[i] = ctx.h[i] ^ v[i] ^ v[i + 16];
         }
         const
          parameterBlock =
            new
             Uint8Array
             ([0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0,
               0]);
         function blake2bInit(outlen, key){
          if(outlen === 0 || outlen > 64)
           throw new Error("Illegal output length, expected 0 < length <= 64");
          if(key.length > 64)
           throw new
                  Error
                  ("Illegal key, expected Uint8Array with 0 < length <= 64");
          const
           ctx =
             {b: new Uint8Array(128),
              h: new Uint32Array(16),
              t: 0,
              c: 0,
              outlen: outlen};
          parameterBlock.fill(0);
          parameterBlock[0] = outlen;
          parameterBlock[1] = key.length;
          parameterBlock[2] = 1;
          parameterBlock[3] = 1;
          for(let i = 0; i < 16; i++)
           ctx.h[i] = BLAKE2B_IV32[i] ^ B2B_GET32(parameterBlock, i * 4);
          if(key.length > 0){blake2bUpdate(ctx, key); ctx.c = 128;}
          return ctx;
         }
         function blake2bUpdate(ctx, input){
          for(let i = 0; i < input.length; i++){
           if(ctx.c === 128){
            ctx.t += ctx.c;
            blake2bCompress(ctx, false);
            ctx.c = 0;
           }
           ctx.b[ctx.c++] = input[i];
          }
         }
         function blake2bFinal(ctx){
          ctx.t += ctx.c;
          while(ctx.c < 128) ctx.b[ctx.c++] = 0;
          blake2bCompress(ctx, true);
          const out = new Uint8Array(ctx.outlen);
          for(let i = 0; i < ctx.outlen; i++)
           out[i] = ctx.h[i >> 2] >> 8 * (i & 3);
          return out;
         }
         return {Init: blake2bInit,
                 Update: blake2bUpdate,
                 Final: blake2bFinal};
        }
        ();
    function caml_ml_string_length(s){return s.length;}
    function caml_string_unsafe_get(s, i){return s.charCodeAt(i);}
    function caml_uint8_array_of_string(s){
     var l = caml_ml_string_length(s), a = new Uint8Array(l), i = 0;
     for(; i < l; i++) a[i] = caml_string_unsafe_get(s, i);
     return a;
    }
    function caml_blake2_create(hashlen, key){
     key = caml_uint8_array_of_string(key);
     if(key.length > 64) key.subarray(0, 64);
     return blake2b.Init(hashlen, key);
    }
    function caml_string_of_jsbytes(x){return x;}
    function blake2_js_for_wasm_create(hashlen, key){
     const key_jsoo_string = caml_string_of_jsbytes(key);
     return caml_blake2_create(hashlen, key_jsoo_string);
    }
    function caml_sub_uint8_array_to_jsbytes(a, i, len){
     var f = String.fromCharCode;
     if(i === 0 && len <= 4096 && len === a.length) return f.apply(null, a);
     var s = "";
     for(; 0 < len; i += 1024, len -= 1024)
      s += f.apply(null, a.subarray(i, i + Math.min(len, 1024)));
     return s;
    }
    function caml_string_of_uint8_array(a){
     return caml_sub_uint8_array_to_jsbytes(a, 0, a.length);
    }
    function caml_blake2_final(ctx, _hashlen){
     var r = blake2b.Final(ctx);
     return caml_string_of_uint8_array(r);
    }
    function caml_jsbytes_of_string(x){return x;}
    function blake2_js_for_wasm_final(ctx, hashlen){
     return caml_jsbytes_of_string(caml_blake2_final(ctx, hashlen));
    }
    function caml_blake2_update(ctx, buf, ofs, len){
     var input = caml_uint8_array_of_string(buf);
     input = input.subarray(ofs, ofs + len);
     blake2b.Update(ctx, input);
     return 0;
    }
    function blake2_js_for_wasm_update(ctx, buf, ofs, len){
     const buf_jsoo_string = caml_string_of_jsbytes(buf);
     return caml_blake2_update(ctx, buf_jsoo_string, ofs, len);
    }
    function caml_js_html_entities(s){
     var entity = /^&#?[0-9a-zA-Z]+;$/;
     if(s.match(entity)){
      var str, temp = document.createElement("p");
      temp.innerHTML = s;
      str = temp.textContent || temp.innerText;
      temp = null;
      return str;
     }
     else
      return null;
    }
    var caml_js_regexps = {amp: /&/g, lt: /</g, quot: /"/g, all: /[&<"]/};
    function caml_js_html_escape(s){
     if(! caml_js_regexps.all.test(s)) return s;
     return s.replace(caml_js_regexps.amp, "&amp;").replace
              (caml_js_regexps.lt, "&lt;").replace
             (caml_js_regexps.quot, "&quot;");
    }
    var
     unix_error =
       ["E2BIG",
        "EACCES",
        "EAGAIN",
        "EBADF",
        "EBUSY",
        "ECHILD",
        "EDEADLK",
        "EDOM",
        "EEXIST",
        "EFAULT",
        "EFBIG",
        "EINTR",
        "EINVAL",
        "EIO",
        "EISDIR",
        "EMFILE",
        "EMLINK",
        "ENAMETOOLONG",
        "ENFILE",
        "ENODEV",
        "ENOENT",
        "ENOEXEC",
        "ENOLCK",
        "ENOMEM",
        "ENOSPC",
        "ENOSYS",
        "ENOTDIR",
        "ENOTEMPTY",
        "ENOTTY",
        "ENXIO",
        "EPERM",
        "EPIPE",
        "ERANGE",
        "EROFS",
        "ESPIPE",
        "ESRCH",
        "EXDEV",
        "EWOULDBLOCK",
        "EINPROGRESS",
        "EALREADY",
        "ENOTSOCK",
        "EDESTADDRREQ",
        "EMSGSIZE",
        "EPROTOTYPE",
        "ENOPROTOOPT",
        "EPROTONOSUPPORT",
        "ESOCKTNOSUPPORT",
        "EOPNOTSUPP",
        "EPFNOSUPPORT",
        "EAFNOSUPPORT",
        "EADDRINUSE",
        "EADDRNOTAVAIL",
        "ENETDOWN",
        "ENETUNREACH",
        "ENETRESET",
        "ECONNABORTED",
        "ECONNRESET",
        "ENOBUFS",
        "EISCONN",
        "ENOTCONN",
        "ESHUTDOWN",
        "ETOOMANYREFS",
        "ETIMEDOUT",
        "ECONNREFUSED",
        "EHOSTDOWN",
        "EHOSTUNREACH",
        "ELOOP",
        "EOVERFLOW"];
    function caml_strerror(errno){
     const util = require("node:util");
     if(errno >= 0){
      const code = unix_error[errno];
      return util.getSystemErrorMap().entries().find(x=>x[1][0] === code)[1]
              [1];
     }
     else
      return util.getSystemErrorMessage(errno);
    }
    var
     zstd_decompress =
       function(){
         var
          ab = ArrayBuffer,
          u8 = Uint8Array,
          u16 = Uint16Array,
          i16 = Int16Array,
          i32 = Int32Array;
         function slc(v, s, e){
          if(u8.prototype.slice) return u8.prototype.slice.call(v, s, e);
          if(s == null || s < 0) s = 0;
          if(e == null || e > v.length) e = v.length;
          var n = new u8(e - s);
          n.set(v.subarray(s, e));
          return n;
         }
         function fill(v, n, s, e){
          if(u8.prototype.fill) return u8.prototype.fill.call(v, n, s, e);
          if(s == null || s < 0) s = 0;
          if(e == null || e > v.length) e = v.length;
          for(; s < e; ++s) v[s] = n;
          return v;
         }
         function cpw(v, t, s, e){
          if(u8.prototype.copyWithin)
           return u8.prototype.copyWithin.call(v, t, s, e);
          if(s == null || s < 0) s = 0;
          if(e == null || e > v.length) e = v.length;
          while(s < e) v[t++] = v[s++];
         }
         var
          ec =
            ["invalid zstd data",
             "window size too large (>2046MB)",
             "invalid block type",
             "FSE accuracy too high",
             "match distance too far back",
             "unexpected EOF"];
         function err(ind, msg, nt){
          var e = new Error(msg || ec[ind]);
          e.code = ind;
          if(! nt) throw e;
          return e;
         }
         function rb(d, b, n){
          var i = 0, o = 0;
          for(; i < n; ++i) o |= d[b++] << (i << 3);
          return o;
         }
         function b4(d, b){
          return (d[b] | d[b + 1] << 8 | d[b + 2] << 16 | d[b + 3] << 24)
                 >>> 0;
         }
         function rzfh(dat, w){
          var n3 = dat[0] | dat[1] << 8 | dat[2] << 16;
          if(n3 === 0x2fb528 && dat[3] === 253){
           var
            flg = dat[4],
            ss = flg >> 5 & 1,
            cc = flg >> 2 & 1,
            df = flg & 3,
            fcf = flg >> 6;
           if(flg & 8) err(0);
           var bt = 6 - ss, db = df === 3 ? 4 : df, di = rb(dat, bt, db);
           bt += db;
           var
            fsb = fcf ? 1 << fcf : ss,
            fss = rb(dat, bt, fsb) + (fcf === 1 && 256),
            ws = fss;
           if(! ss){
            var wb = 1 << 10 + (dat[5] >> 3);
            ws = wb + (wb >> 3) * (dat[5] & 7);
           }
           if(ws > 2145386496) err(1);
           var buf = new u8((w === 1 ? fss || ws : w ? 0 : ws) + 12);
           buf[0] = 1, buf[4] = 4, buf[8] = 8;
           return {b: bt + fsb,
                   y: 0,
                   l: 0,
                   d: di,
                   w: w && w !== 1 ? w : buf.subarray(12),
                   e: ws,
                   o: new i32(buf.buffer, 0, 3),
                   u: fss,
                   c: cc,
                   m: Math.min(131072, ws)};
          }
          else if((n3 >> 4 | dat[3] << 20) === 0x184d2a5)
           return b4(dat, 4) + 8;
          err(0);
         }
         function msb(val){
          var bits = 0;
          for(; 1 << bits <= val; ++bits) ;
          return bits - 1;
         }
         function rfse(dat, bt, mal){
          var tpos = (bt << 3) + 4, al = (dat[bt] & 15) + 5;
          if(al > mal) err(3);
          var
           sz = 1 << al,
           probs = sz,
           sym = - 1,
           re = - 1,
           i = - 1,
           ht = sz,
           buf = new ab(512 + (sz << 2)),
           freq = new i16(buf, 0, 256),
           dstate = new u16(buf, 0, 256),
           nstate = new u16(buf, 512, sz),
           bb1 = 512 + (sz << 1),
           syms = new u8(buf, bb1, sz),
           nbits = new u8(buf, bb1 + sz);
          while(sym < 255 && probs > 0){
           var
            bits = msb(probs + 1),
            cbt = tpos >> 3,
            msk = (1 << bits + 1) - 1,
            val =
              (dat[cbt] | dat[cbt + 1] << 8 | dat[cbt + 2] << 16)
              >> (tpos & 7)
              & msk,
            msk1fb = (1 << bits) - 1,
            msv = msk - probs - 1,
            sval = val & msk1fb;
           if(sval < msv)
            tpos += bits, val = sval;
           else{tpos += bits + 1; if(val > msk1fb) val -= msv;}
           freq[++sym] = --val;
           if(val === - 1){probs += val; syms[--ht] = sym;} else probs -= val;
           if(! val)
            do{
             var rbt = tpos >> 3;
             re = (dat[rbt] | dat[rbt + 1] << 8) >> (tpos & 7) & 3;
             tpos += 2;
             sym += re;
            }
            while
             (re === 3);
          }
          if(sym > 255 || probs) err(0);
          var sympos = 0, sstep = (sz >> 1) + (sz >> 3) + 3, smask = sz - 1;
          for(var s = 0; s <= sym; ++s){
           var sf = freq[s];
           if(sf < 1){dstate[s] = - sf; continue;}
           for(i = 0; i < sf; ++i){
            syms[sympos] = s;
            do sympos = sympos + sstep & smask;while(sympos >= ht);
           }
          }
          if(sympos) err(0);
          for(i = 0; i < sz; ++i){
           var ns = dstate[syms[i]]++, nb = nbits[i] = al - msb(ns);
           nstate[i] = (ns << nb) - sz;
          }
          return [tpos + 7 >> 3, {b: al, s: syms, n: nbits, t: nstate}];
         }
         function rhu(dat, bt){
          var
           i = 0,
           wc = - 1,
           buf = new u8(292),
           hb = dat[bt],
           hw = buf.subarray(0, 256),
           rc = buf.subarray(256, 268),
           ri = new u16(buf.buffer, 268);
          if(hb < 128){
           var _a = rfse(dat, bt + 1, 6), ebt = _a[0], fdt = _a[1];
           bt += hb;
           var epos = ebt << 3, lb = dat[bt];
           if(! lb) err(0);
           var
            st1 = 0,
            st2 = 0,
            btr1 = fdt.b,
            btr2 = btr1,
            fpos = (++bt << 3) - 8 + msb(lb);
           for(;;){
            fpos -= btr1;
            if(fpos < epos) break;
            var cbt = fpos >> 3;
            st1 +=
             (dat[cbt] | dat[cbt + 1] << 8) >> (fpos & 7) & (1 << btr1) - 1;
            hw[++wc] = fdt.s[st1];
            fpos -= btr2;
            if(fpos < epos) break;
            cbt = fpos >> 3;
            st2 +=
             (dat[cbt] | dat[cbt + 1] << 8) >> (fpos & 7) & (1 << btr2) - 1;
            hw[++wc] = fdt.s[st2];
            btr1 = fdt.n[st1];
            st1 = fdt.t[st1];
            btr2 = fdt.n[st2];
            st2 = fdt.t[st2];
           }
           if(++wc > 255) err(0);
          }
          else{
           wc = hb - 127;
           for(; i < wc; i += 2){
            var byte = dat[++bt];
            hw[i] = byte >> 4;
            hw[i + 1] = byte & 15;
           }
           ++bt;
          }
          var wes = 0;
          for(i = 0; i < wc; ++i){
           var wt = hw[i];
           if(wt > 11) err(0);
           wes += wt && 1 << wt - 1;
          }
          var mb = msb(wes) + 1, ts = 1 << mb, rem = ts - wes;
          if(rem & rem - 1) err(0);
          hw[wc++] = msb(rem) + 1;
          for(i = 0; i < wc; ++i){
           var wt = hw[i];
           ++rc[hw[i] = wt && mb + 1 - wt];
          }
          var
           hbuf = new u8(ts << 1),
           syms = hbuf.subarray(0, ts),
           nb = hbuf.subarray(ts);
          ri[mb] = 0;
          for(i = mb; i > 0; --i){
           var pv = ri[i];
           fill(nb, i, pv, ri[i - 1] = pv + rc[i] * (1 << mb - i));
          }
          if(ri[0] !== ts) err(0);
          for(i = 0; i < wc; ++i){
           var bits = hw[i];
           if(bits){
            var code = ri[bits];
            fill(syms, i, code, ri[bits] = code + (1 << mb - bits));
           }
          }
          return [bt, {n: nb, b: mb, s: syms}];
         }
         var
          dllt =
            rfse
              (new
                u8
                ([81,
                  16,
                  99,
                  140,
                  49,
                  198,
                  24,
                  99,
                  12,
                  33,
                  196,
                  24,
                  99,
                  102,
                  102,
                  134,
                  70,
                  146,
                  4]),
               0,
               6)
             [1],
          dmlt =
            rfse
              (new
                u8
                ([33,
                  20,
                  196,
                  24,
                  99,
                  140,
                  33,
                  132,
                  16,
                  66,
                  8,
                  33,
                  132,
                  16,
                  66,
                  8,
                  33,
                  68,
                  68,
                  68,
                  68,
                  68,
                  68,
                  68,
                  68,
                  36,
                  9]),
               0,
               6)
             [1],
          doct =
            rfse
              (new u8([32, 132, 16, 66, 102, 70, 68, 68, 68, 68, 36, 73, 2]),
               0,
               5)
             [1];
         function b2bl(b, s){
          var len = b.length, bl = new i32(len);
          for(var i = 0; i < len; ++i){bl[i] = s; s += 1 << b[i];}
          return bl;
         }
         var
          llb =
            new
             u8
             (new
               i32
               ([0,
                 0,
                 0,
                 0,
                 16843009,
                 50528770,
                 134678020,
                 202050057,
                 269422093]).buffer,
              0,
              36),
          llbl = b2bl(llb, 0),
          mlb =
            new
             u8
             (new
               i32
               ([0,
                 0,
                 0,
                 0,
                 0,
                 0,
                 0,
                 0,
                 16843009,
                 50528770,
                 117769220,
                 185207048,
                 252579084,
                 16]).buffer,
              0,
              53),
          mlbl = b2bl(mlb, 3);
         function dhu(dat, out, hu){
          var
           len = dat.length,
           ss = out.length,
           lb = dat[len - 1],
           msk = (1 << hu.b) - 1,
           eb = - hu.b;
          if(! lb) err(0);
          var
           st = 0,
           btr = hu.b,
           pos = (len << 3) - 8 + msb(lb) - btr,
           i = - 1;
          while(pos > eb && i < ss){
           var
            cbt = pos >> 3,
            val =
              (dat[cbt] | dat[cbt + 1] << 8 | dat[cbt + 2] << 16) >> (pos & 7);
           st = (st << btr | val) & msk;
           out[++i] = hu.s[st];
           pos -= btr = hu.n[st];
          }
          if(pos !== eb || i + 1 !== ss) err(0);
         }
         function dhu4(dat, out, hu){
          var
           bt = 6,
           ss = out.length,
           sz1 = ss + 3 >> 2,
           sz2 = sz1 << 1,
           sz3 = sz1 + sz2;
          dhu
           (dat.subarray(bt, bt += dat[0] | dat[1] << 8),
            out.subarray(0, sz1),
            hu);
          dhu
           (dat.subarray(bt, bt += dat[2] | dat[3] << 8),
            out.subarray(sz1, sz2),
            hu);
          dhu
           (dat.subarray(bt, bt += dat[4] | dat[5] << 8),
            out.subarray(sz2, sz3),
            hu);
          dhu(dat.subarray(bt), out.subarray(sz3), hu);
         }
         function rzb(dat, st, out){
          var _a, bt = st.b, b0 = dat[bt], btype = b0 >> 1 & 3;
          st.l = b0 & 1;
          var
           sz = b0 >> 3 | dat[bt + 1] << 5 | dat[bt + 2] << 13,
           ebt = (bt += 3) + sz;
          if(btype === 1){
           if(bt >= dat.length) return;
           st.b = bt + 1;
           if(out){fill(out, dat[bt], st.y, st.y += sz); return out;}
           return fill(new u8(sz), dat[bt]);
          }
          if(ebt > dat.length) return;
          if(btype === 0){
           st.b = ebt;
           if(out){
            out.set(dat.subarray(bt, ebt), st.y);
            st.y += sz;
            return out;
           }
           return slc(dat, bt, ebt);
          }
          if(btype === 2){
           var
            b3 = dat[bt],
            lbt = b3 & 3,
            sf = b3 >> 2 & 3,
            lss = b3 >> 4,
            lcs = 0,
            s4 = 0;
           if(lbt < 2)
            if(sf & 1)
             lss |= dat[++bt] << 4 | (sf & 2 && dat[++bt] << 12);
            else
             lss = b3 >> 3;
           else{
            s4 = sf;
            if(sf < 2)
             lss |= (dat[++bt] & 63) << 4,
             lcs = dat[bt] >> 6 | dat[++bt] << 2;
            else if(sf === 2)
             lss |= dat[++bt] << 4 | (dat[++bt] & 3) << 12,
             lcs = dat[bt] >> 2 | dat[++bt] << 6;
            else
             lss |= dat[++bt] << 4 | (dat[++bt] & 63) << 12,
             lcs = dat[bt] >> 6 | dat[++bt] << 2 | dat[++bt] << 10;
           }
           ++bt;
           var
            buf = out ? out.subarray(st.y, st.y + st.m) : new u8(st.m),
            spl = buf.length - lss;
           if(lbt === 0)
            buf.set(dat.subarray(bt, bt += lss), spl);
           else if(lbt === 1)
            fill(buf, dat[bt++], spl);
           else{
            var hu = st.h;
            if(lbt === 2){
             var hud = rhu(dat, bt);
             lcs += bt - (bt = hud[0]);
             st.h = hu = hud[1];
            }
            else if(! hu) err(0);
            (s4 ? dhu4 : dhu)
             (dat.subarray(bt, bt += lcs), buf.subarray(spl), hu);
           }
           var ns = dat[bt++];
           if(ns){
            if(ns === 255)
             ns = (dat[bt++] | dat[bt++] << 8) + 0x7f00;
            else if(ns > 127) ns = ns - 128 << 8 | dat[bt++];
            var scm = dat[bt++];
            if(scm & 3) err(0);
            var dts = [dmlt, doct, dllt];
            for(var i = 2; i > - 1; --i){
             var md = scm >> (i << 1) + 2 & 3;
             if(md === 1){
              var rbuf = new u8([0, 0, dat[bt++]]);
              dts[i] =
               {s: rbuf.subarray(2, 3),
                n: rbuf.subarray(0, 1),
                t: new u16(rbuf.buffer, 0, 1),
                b: 0};
             }
             else if(md === 2)
              _a = rfse(dat, bt, 9 - (i & 1)), bt = _a[0], dts[i] = _a[1];
             else if(md === 3){if(! st.t) err(0); dts[i] = st.t[i];}
            }
            var
             _b = st.t = dts,
             mlt = _b[0],
             oct = _b[1],
             llt = _b[2],
             lb = dat[ebt - 1];
            if(! lb) err(0);
            var
             spos = (ebt << 3) - 8 + msb(lb) - llt.b,
             cbt = spos >> 3,
             oubt = 0,
             lst =
               (dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7) & (1 << llt.b) - 1;
            cbt = (spos -= oct.b) >> 3;
            var
             ost =
               (dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7) & (1 << oct.b) - 1;
            cbt = (spos -= mlt.b) >> 3;
            var
             mst =
               (dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7) & (1 << mlt.b) - 1;
            for(++ns; --ns;){
             var
              llc = llt.s[lst],
              lbtr = llt.n[lst],
              mlc = mlt.s[mst],
              mbtr = mlt.n[mst],
              ofc = oct.s[ost],
              obtr = oct.n[ost];
             cbt = (spos -= ofc) >> 3;
             var
              ofp = 1 << ofc,
              off =
                ofp
                +
                 ((dat[cbt] | dat[cbt + 1] << 8 | dat[cbt + 2] << 16
                 | dat[cbt + 3] << 24)
                 >>> (spos & 7)
                 & ofp - 1);
             cbt = (spos -= mlb[mlc]) >> 3;
             var
              ml =
                mlbl[mlc]
                +
                 ((dat[cbt] | dat[cbt + 1] << 8 | dat[cbt + 2] << 16)
                 >> (spos & 7)
                 & (1 << mlb[mlc]) - 1);
             cbt = (spos -= llb[llc]) >> 3;
             var
              ll =
                llbl[llc]
                +
                 ((dat[cbt] | dat[cbt + 1] << 8 | dat[cbt + 2] << 16)
                 >> (spos & 7)
                 & (1 << llb[llc]) - 1);
             cbt = (spos -= lbtr) >> 3;
             lst =
              llt.t[lst]
              +
               ((dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7)
               & (1 << lbtr) - 1);
             cbt = (spos -= mbtr) >> 3;
             mst =
              mlt.t[mst]
              +
               ((dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7)
               & (1 << mbtr) - 1);
             cbt = (spos -= obtr) >> 3;
             ost =
              oct.t[ost]
              +
               ((dat[cbt] | dat[cbt + 1] << 8) >> (spos & 7)
               & (1 << obtr) - 1);
             if(off > 3){
              st.o[2] = st.o[1];
              st.o[1] = st.o[0];
              st.o[0] = off -= 3;
             }
             else{
              var idx = off - (ll !== 0);
              if(idx){
               off = idx === 3 ? st.o[0] - 1 : st.o[idx];
               if(idx > 1) st.o[2] = st.o[1];
               st.o[1] = st.o[0];
               st.o[0] = off;
              }
              else
               off = st.o[0];
             }
             for(var i = 0; i < ll; ++i) buf[oubt + i] = buf[spl + i];
             oubt += ll, spl += ll;
             var stin = oubt - off;
             if(stin < 0){
              var len = - stin, bs = st.e + stin;
              if(len > ml) len = ml;
              for(var i = 0; i < len; ++i) buf[oubt + i] = st.w[bs + i];
              oubt += len, ml -= len, stin = 0;
             }
             for(var i = 0; i < ml; ++i) buf[oubt + i] = buf[stin + i];
             oubt += ml;
            }
            if(oubt !== spl)
             while(spl < buf.length) buf[oubt++] = buf[spl++];
            else
             oubt = buf.length;
            if(out) st.y += oubt; else buf = slc(buf, 0, oubt);
           }
           else if(out){
            st.y += lss;
            if(spl) for(var i = 0; i < lss; ++i) buf[i] = buf[spl + i];
           }
           else if(spl) buf = slc(buf, spl);
           st.b = ebt;
           return buf;
          }
          err(2);
         }
         function cct(bufs, ol){
          if(bufs.length === 1) return bufs[0];
          var buf = new u8(ol);
          for(var i = 0, b = 0; i < bufs.length; ++i){
           var chk = bufs[i];
           buf.set(chk, b);
           b += chk.length;
          }
          return buf;
         }
         return function(dat, buf){
          var bt = 0, bufs = [], nb = + ! buf, ol = 0;
          while(dat.length){
           var st = rzfh(dat, nb || buf);
           if(typeof st === "object"){
            if(nb){
             buf = null;
             if(st.w.length === st.u){bufs.push(buf = st.w); ol += st.u;}
            }
            else{bufs.push(buf); st.e = 0;}
            while(! st.l){
             var blk = rzb(dat, st, buf);
             if(! blk) err(5);
             if(buf)
              st.e = st.y;
             else{
              bufs.push(blk);
              ol += blk.length;
              cpw(st.w, 0, blk.length);
              st.w.set(blk, st.w.length - blk.length);
             }
            }
            bt = st.b + st.c * 4;
           }
           else
            bt = st;
           dat = dat.subarray(bt);
          }
          return cct(bufs, ol);};
        }
        ();
    return {zstd_decompress: zstd_decompress,
            unix_error: unix_error,
            caml_strerror: caml_strerror,
            caml_js_html_escape: caml_js_html_escape,
            caml_js_html_entities: caml_js_html_entities,
            blake2_js_for_wasm_update: blake2_js_for_wasm_update,
            blake2_js_for_wasm_final: blake2_js_for_wasm_final,
            blake2_js_for_wasm_create: blake2_js_for_wasm_create};
   }
   (globalThis))
({"link":[["runtime-19fd5996",0],["prelude-d7e4b000",0],["stdlib-40b2cbc1",[]],["yojson-26490c3b",[2]],["str-127c9347",[2]],["unix-764e5c28",[2]],["cel-d02c744b",[2,3,4,5]],["jsoo_runtime-1e55b178",[2]],["js_of_ocaml-97350c3b",[2,7]],["ocamlcommon-8a36ea94",[2]],["astlib-a0a06e10",[2,9]],["ppxlib_ast-87543642",[2,10]],["ocaml_shadow-0ebf7fe4",[]],["ppxlib_print_diff-fec2dd44",[2]],["ppx_derivers-a78b6a53",[2]],["ppxlib_traverse_builtins-8299787c",[2]],["sexplib0-90abbc8a",[2]],["stdppx-884388c1",[2,16]],["ppxlib-b0b76339",[2,10,11,12,13,14,15,16,17]],["ppx_js-7c7f9733",[2,9,11,18]],["ppx_js_rewriter-c5d03a14",[19]],["dune__exe__Cel_wasm-540f62a4",[2,4,6,8]],["std_exit-73ee5fad",[2]],["start-9c00fdee",0]],"generated":(a=>{var
c=a,b=a?.module?.export||a;return{"env":{"caml_ba_kind_of_typed_array":()=>{throw new
Error("caml_ba_kind_of_typed_array not implemented")},"caml_dynlink_add_primitive":()=>{throw new
Error("caml_dynlink_add_primitive not implemented")},"caml_dynlink_close_lib":()=>{throw new
Error("caml_dynlink_close_lib not implemented")},"caml_dynlink_get_bytecode_sections":()=>{throw new
Error("caml_dynlink_get_bytecode_sections not implemented")},"caml_dynlink_get_current_libs":()=>{throw new
Error("caml_dynlink_get_current_libs not implemented")},"caml_dynlink_lookup_symbol":()=>{throw new
Error("caml_dynlink_lookup_symbol not implemented")},"caml_dynlink_open_lib":()=>{throw new
Error("caml_dynlink_open_lib not implemented")},"caml_exn_with_js_backtrace":()=>{throw new
Error("caml_exn_with_js_backtrace not implemented")},"caml_int64_create_lo_mi_hi":()=>{throw new
Error("caml_int64_create_lo_mi_hi not implemented")},"caml_jsoo_flags_effects":()=>{throw new
Error("caml_jsoo_flags_effects not implemented")},"caml_list_mount_point":()=>{throw new
Error("caml_list_mount_point not implemented")},"caml_ml_set_channel_output":()=>{throw new
Error("caml_ml_set_channel_output not implemented")},"caml_ml_set_channel_refill":()=>{throw new
Error("caml_ml_set_channel_refill not implemented")},"caml_realloc_global":()=>{throw new
Error("caml_realloc_global not implemented")},"caml_unix_accept":()=>{throw new
Error("caml_unix_accept not implemented")},"caml_unix_alarm":()=>{throw new
Error("caml_unix_alarm not implemented")},"caml_unix_bind":()=>{throw new
Error("caml_unix_bind not implemented")},"caml_unix_chown":()=>{throw new
Error("caml_unix_chown not implemented")},"caml_unix_chroot":()=>{throw new
Error("caml_unix_chroot not implemented")},"caml_unix_clear_close_on_exec":()=>{throw new
Error("caml_unix_clear_close_on_exec not implemented")},"caml_unix_clear_nonblock":()=>{throw new
Error("caml_unix_clear_nonblock not implemented")},"caml_unix_connect":()=>{throw new
Error("caml_unix_connect not implemented")},"caml_unix_dup":()=>{throw new
Error("caml_unix_dup not implemented")},"caml_unix_dup2":()=>{throw new
Error("caml_unix_dup2 not implemented")},"caml_unix_environment":()=>{throw new
Error("caml_unix_environment not implemented")},"caml_unix_environment_unsafe":()=>{throw new
Error("caml_unix_environment_unsafe not implemented")},"caml_unix_execv":()=>{throw new
Error("caml_unix_execv not implemented")},"caml_unix_execve":()=>{throw new
Error("caml_unix_execve not implemented")},"caml_unix_execvp":()=>{throw new
Error("caml_unix_execvp not implemented")},"caml_unix_execvpe":()=>{throw new
Error("caml_unix_execvpe not implemented")},"caml_unix_fchown":()=>{throw new
Error("caml_unix_fchown not implemented")},"caml_unix_fork":()=>{throw new
Error("caml_unix_fork not implemented")},"caml_unix_getaddrinfo":()=>{throw new
Error("caml_unix_getaddrinfo not implemented")},"caml_unix_getegid":()=>{throw new
Error("caml_unix_getegid not implemented")},"caml_unix_getgrgid":()=>{throw new
Error("caml_unix_getgrgid not implemented")},"caml_unix_getgroups":()=>{throw new
Error("caml_unix_getgroups not implemented")},"caml_unix_gethostbyaddr":()=>{throw new
Error("caml_unix_gethostbyaddr not implemented")},"caml_unix_gethostbyname":()=>{throw new
Error("caml_unix_gethostbyname not implemented")},"caml_unix_gethostname":()=>{throw new
Error("caml_unix_gethostname not implemented")},"caml_unix_getitimer":()=>{throw new
Error("caml_unix_getitimer not implemented")},"caml_unix_getlogin":()=>{throw new
Error("caml_unix_getlogin not implemented")},"caml_unix_getnameinfo":()=>{throw new
Error("caml_unix_getnameinfo not implemented")},"caml_unix_getpeername":()=>{throw new
Error("caml_unix_getpeername not implemented")},"caml_unix_getpid":()=>{throw new
Error("caml_unix_getpid not implemented")},"caml_unix_getppid":()=>{throw new
Error("caml_unix_getppid not implemented")},"caml_unix_getprotobyname":()=>{throw new
Error("caml_unix_getprotobyname not implemented")},"caml_unix_getprotobynumber":()=>{throw new
Error("caml_unix_getprotobynumber not implemented")},"caml_unix_getservbyname":()=>{throw new
Error("caml_unix_getservbyname not implemented")},"caml_unix_getservbyport":()=>{throw new
Error("caml_unix_getservbyport not implemented")},"caml_unix_getsockname":()=>{throw new
Error("caml_unix_getsockname not implemented")},"caml_unix_getsockopt":()=>{throw new
Error("caml_unix_getsockopt not implemented")},"caml_unix_initgroups":()=>{throw new
Error("caml_unix_initgroups not implemented")},"caml_unix_kill":()=>{throw new
Error("caml_unix_kill not implemented")},"caml_unix_listen":()=>{throw new
Error("caml_unix_listen not implemented")},"caml_unix_lockf":()=>{throw new
Error("caml_unix_lockf not implemented")},"caml_unix_map_file_bytecode":()=>{throw new
Error("caml_unix_map_file_bytecode not implemented")},"caml_unix_mkfifo":()=>{throw new
Error("caml_unix_mkfifo not implemented")},"caml_unix_nice":()=>{throw new
Error("caml_unix_nice not implemented")},"caml_unix_pipe":()=>{throw new
Error("caml_unix_pipe not implemented")},"caml_unix_putenv":()=>{throw new
Error("caml_unix_putenv not implemented")},"caml_unix_realpath":()=>{throw new
Error("caml_unix_realpath not implemented")},"caml_unix_recv":()=>{throw new
Error("caml_unix_recv not implemented")},"caml_unix_recvfrom":()=>{throw new
Error("caml_unix_recvfrom not implemented")},"caml_unix_select":()=>{throw new
Error("caml_unix_select not implemented")},"caml_unix_send":()=>{throw new
Error("caml_unix_send not implemented")},"caml_unix_sendto":()=>{throw new
Error("caml_unix_sendto not implemented")},"caml_unix_set_close_on_exec":()=>{throw new
Error("caml_unix_set_close_on_exec not implemented")},"caml_unix_set_nonblock":()=>{throw new
Error("caml_unix_set_nonblock not implemented")},"caml_unix_setgid":()=>{throw new
Error("caml_unix_setgid not implemented")},"caml_unix_setgroups":()=>{throw new
Error("caml_unix_setgroups not implemented")},"caml_unix_setitimer":()=>{throw new
Error("caml_unix_setitimer not implemented")},"caml_unix_setsid":()=>{throw new
Error("caml_unix_setsid not implemented")},"caml_unix_setsockopt":()=>{throw new
Error("caml_unix_setsockopt not implemented")},"caml_unix_setuid":()=>{throw new
Error("caml_unix_setuid not implemented")},"caml_unix_shutdown":()=>{throw new
Error("caml_unix_shutdown not implemented")},"caml_unix_sigpending":()=>{throw new
Error("caml_unix_sigpending not implemented")},"caml_unix_sigprocmask":()=>{throw new
Error("caml_unix_sigprocmask not implemented")},"caml_unix_sigsuspend":()=>{throw new
Error("caml_unix_sigsuspend not implemented")},"caml_unix_sleep":()=>{throw new
Error("caml_unix_sleep not implemented")},"caml_unix_socket":()=>{throw new
Error("caml_unix_socket not implemented")},"caml_unix_socketpair":()=>{throw new
Error("caml_unix_socketpair not implemented")},"caml_unix_spawn":()=>{throw new
Error("caml_unix_spawn not implemented")},"caml_unix_string_of_inet_addr":()=>{throw new
Error("caml_unix_string_of_inet_addr not implemented")},"caml_unix_tcdrain":()=>{throw new
Error("caml_unix_tcdrain not implemented")},"caml_unix_tcflow":()=>{throw new
Error("caml_unix_tcflow not implemented")},"caml_unix_tcflush":()=>{throw new
Error("caml_unix_tcflush not implemented")},"caml_unix_tcgetattr":()=>{throw new
Error("caml_unix_tcgetattr not implemented")},"caml_unix_tcsendbreak":()=>{throw new
Error("caml_unix_tcsendbreak not implemented")},"caml_unix_tcsetattr":()=>{throw new
Error("caml_unix_tcsetattr not implemented")},"caml_unix_umask":()=>{throw new
Error("caml_unix_umask not implemented")},"caml_unix_wait":()=>{throw new
Error("caml_unix_wait not implemented")},"caml_unix_waitpid":()=>{throw new
Error("caml_unix_waitpid not implemented")},"caml_unmount":()=>{throw new
Error("caml_unmount not implemented")}},"Js_of_ocaml__Js.fragments":{"fun_call_1":(a,b)=>a(b),"get_Array":a=>a.Array,"get_Date":a=>a.Date,"get_Error":a=>a.Error,"get_JSON":a=>a.JSON,"get_Math":a=>a.Math,"get_Object":a=>a.Object,"get_RegExp":a=>a.RegExp,"get_String":a=>a.String,"get_decodeURI":a=>a.decodeURI,"get_decodeURIComponent":a=>a.decodeURIComponent,"get_encodeURI":a=>a.encodeURI,"get_encodeURIComponent":a=>a.encodeURIComponent,"get_escape":a=>a.escape,"get_isNaN":a=>a.isNaN,"get_length":a=>a.length,"get_message":a=>a.message,"get_name":a=>a.name,"get_parseFloat":a=>a.parseFloat,"get_parseInt":a=>a.parseInt,"get_stack":a=>a.stack,"get_unescape":a=>a.unescape,"js_expr_12c48ca8":()=>a,"js_expr_21711c2a":()=>b,"js_expr_26f07992":()=>null,"js_expr_28647a4c":()=>false,"js_expr_34edcf72":()=>true,"js_expr_ba692c1":()=>undefined,"meth_call_0_toString":a=>a.toString(),"meth_call_1_forEach":(a,b)=>a.forEach(b),"meth_call_1_keys":(a,b)=>a.keys(b),"meth_call_1_map":(a,b)=>a.map(b)},"Js_of_ocaml__Dom.fragments":{"call_1":(a,b,c)=>a.call(b,c),"get_CustomEvent":a=>a.CustomEvent,"get_addEventListener":a=>a.addEventListener,"get_length":a=>a.length,"get_nodeType":a=>a.nodeType,"get_srcElement":a=>a.srcElement,"get_target":a=>a.target,"meth_call_0_preventDefault":a=>a.preventDefault(),"meth_call_1_appendChild":(a,b)=>a.appendChild(b),"meth_call_1_concat":(a,b)=>a.concat(b),"meth_call_1_item":(a,b)=>a.item(b),"meth_call_1_removeChild":(a,b)=>a.removeChild(b),"meth_call_2_attachEvent":(a,b,c)=>a.attachEvent(b,c),"meth_call_2_detachEvent":(a,b,c)=>a.detachEvent(b,c),"meth_call_2_insertBefore":(a,b,c)=>a.insertBefore(b,c),"meth_call_2_replaceChild":(a,b,c)=>a.replaceChild(b,c),"meth_call_3_addEventListener":(a,b,c,d)=>a.addEventListener(b,c,d),"meth_call_3_removeEventListener":(a,b,c,d)=>a.removeEventListener(b,c,d),"new_2":(a,b,c)=>new
a(b,c),"obj_0":()=>({}),"obj_1":()=>({}),"set_bubbles":(a,b)=>a.bubbles=b,"set_cancelable":(a,b)=>a.cancelable=b,"set_capture":(a,b)=>a.capture=b,"set_detail":(a,b)=>a.detail=b,"set_once":(a,b)=>a.once=b,"set_passive":(a,b)=>a.passive=b},"Js_of_ocaml__Typed_array.fragments":{"get_ArrayBuffer":a=>a.ArrayBuffer,"get_DataView":a=>a.DataView,"get_Float32Array":a=>a.Float32Array,"get_Float64Array":a=>a.Float64Array,"get_Int16Array":a=>a.Int16Array,"get_Int32Array":a=>a.Int32Array,"get_Int8Array":a=>a.Int8Array,"get_Uint16Array":a=>a.Uint16Array,"get_Uint32Array":a=>a.Uint32Array,"get_Uint8Array":a=>a.Uint8Array,"new_1":(a,b)=>new
a(b)},"Js_of_ocaml__File.fragments":{"get_Blob":a=>a.Blob,"get_Document":a=>a.Document,"get_FileReader":a=>a.FileReader,"get_fileName":a=>a.fileName,"get_name":a=>a.name,"new_2":(a,b,c)=>new
a(b,c)},"Js_of_ocaml__Dom_html.fragments":{"fun_call_1":(a,b)=>a(b),"get_HTMLElement":a=>a.HTMLElement,"get_KeyboardEvent":a=>a.KeyboardEvent,"get_MessageEvent":a=>a.MessageEvent,"get_MouseEvent":a=>a.MouseEvent,"get_MouseScrollEvent":a=>a.MouseScrollEvent,"get_PopStateEvent":a=>a.PopStateEvent,"get_WheelEvent":a=>a.WheelEvent,"get_body":a=>a.body,"get_button":a=>a.button,"get_charCode":a=>a.charCode,"get_clientLeft":a=>a.clientLeft,"get_clientTop":a=>a.clientTop,"get_clientX":a=>a.clientX,"get_clientY":a=>a.clientY,"get_code":a=>a.code,"get_document":a=>a.document,"get_documentElement":a=>a.documentElement,"get_getContext":a=>a.getContext,"get_history":a=>a.history,"get_key":a=>a.key,"get_keyCode":a=>a.keyCode,"get_left":a=>a.left,"get_length":a=>a.length,"get_location":a=>a.location,"get_mozRequestAnimationFrame":a=>a.mozRequestAnimationFrame,"get_msRequestAnimationFrame":a=>a.msRequestAnimationFrame,"get_name":a=>a.name,"get_oRequestAnimationFrame":a=>a.oRequestAnimationFrame,"get_origin":a=>a.origin,"get_pageX":a=>a.pageX,"get_pageY":a=>a.pageY,"get_placeholder":a=>a.placeholder,"get_pushState":a=>a.pushState,"get_relatedTarget":a=>a.relatedTarget,"get_requestAnimationFrame":a=>a.requestAnimationFrame,"get_required":a=>a.required,"get_scrollLeft":a=>a.scrollLeft,"get_scrollTop":a=>a.scrollTop,"get_stopPropagation":a=>a.stopPropagation,"get_tagName":a=>a.tagName,"get_top":a=>a.top,"get_webkitRequestAnimationFrame":a=>a.webkitRequestAnimationFrame,"get_wheelDelta":a=>a.wheelDelta,"get_wheelDeltaX":a=>a.wheelDeltaX,"get_wheelDeltaY":a=>a.wheelDeltaY,"get_which":a=>a.which,"js_expr_4c8b1c6":()=>[].slice,"meth_call_0_getBoundingClientRect":a=>a.getBoundingClientRect(),"meth_call_0_getTime":a=>a.getTime(),"meth_call_0_stopPropagation":a=>a.stopPropagation(),"meth_call_0_toLowerCase":a=>a.toLowerCase(),"meth_call_1_call":(a,b)=>a.call(b),"meth_call_1_charCodeAt":(a,b)=>a.charCodeAt(b),"meth_call_1_clearTimeout":(a,b)=>a.clearTimeout(b),"meth_call_1_createElement":(a,b)=>a.createElement(b),"meth_call_1_getElementById":(a,b)=>a.getElementById(b),"meth_call_1_join":(a,b)=>a.join(b),"meth_call_1_push":(a,b)=>a.push(b),"meth_call_2_push":(a,b,c)=>a.push(b,c),"meth_call_2_setTimeout":(a,b,c)=>a.setTimeout(b,c),"meth_call_3_push":(a,b,c,d)=>a.push(b,c,d),"new_0":a=>new
a(),"set_cancelBubble":(a,b)=>a.cancelBubble=b,"set_name":(a,b)=>a.name=b,"set_type":(a,b)=>a.type=b},"Js_of_ocaml__Form.fragments":{"get_FormData":a=>a.FormData,"get_checked":a=>a.checked,"get_disabled":a=>a.disabled,"get_elements":a=>a.elements,"get_files":a=>a.files,"get_length":a=>a.length,"get_multiple":a=>a.multiple,"get_name":a=>a.name,"get_options":a=>a.options,"get_selected":a=>a.selected,"get_type":a=>a.type,"get_value":a=>a.value,"meth_call_0_toLowerCase":a=>a.toLowerCase(),"meth_call_1_item":(a,b)=>a.item(b),"meth_call_2_append":(a,b,c)=>a.append(b,c),"new_0":a=>new
a()},"Js_of_ocaml__Worker.fragments":{"get_Worker":a=>a.Worker,"get_data":a=>a.data,"get_importScripts":a=>a.importScripts,"get_onmessage":a=>a.onmessage,"get_postMessage":a=>a.postMessage,"meth_call_1_postMessage":(a,b)=>a.postMessage(b),"new_1":(a,b)=>new
a(b),"set_onmessage":(a,b)=>a.onmessage=b},"Js_of_ocaml__WebSockets.fragments":{"get_WebSocket":a=>a.WebSocket},"Js_of_ocaml__WebGL.fragments":{"meth_call_1_getContext":(a,b)=>a.getContext(b),"meth_call_2_getContext":(a,b,c)=>a.getContext(b,c),"obj_2":(a,b,c,d,e,f,g,h)=>({alpha:a,depth:b,stencil:c,antialias:d,premultipliedAlpha:e,preserveDrawingBuffer:f,preferLowPowerToHighPerformance:g,failIfMajorPerformanceCaveat:h})},"Js_of_ocaml__Regexp.fragments":{"get_ignoreCase":a=>a.ignoreCase,"get_index":a=>a.index,"get_length":a=>a.length,"get_multiline":a=>a.multiline,"get_source":a=>a.source,"meth_call_1_exec":(a,b)=>a.exec(b),"meth_call_1_split":(a,b)=>a.split(b),"meth_call_2_replace":(a,b,c)=>a.replace(b,c),"meth_call_2_split":(a,b,c)=>a.split(b,c),"new_2":(a,b,c)=>new
a(b,c),"set_lastIndex":(a,b)=>a.lastIndex=b},"Js_of_ocaml__Url.fragments":{"get_hash":a=>a.hash,"get_hostname":a=>a.hostname,"get_href":a=>a.href,"get_length":a=>a.length,"get_location":a=>a.location,"get_pathname":a=>a.pathname,"get_port":a=>a.port,"get_protocol":a=>a.protocol,"get_search":a=>a.search,"meth_call_0_toLowerCase":a=>a.toLowerCase(),"meth_call_1_charAt":(a,b)=>a.charAt(b),"meth_call_1_exec":(a,b)=>a.exec(b),"meth_call_1_indexOf":(a,b)=>a.indexOf(b),"meth_call_1_slice":(a,b)=>a.slice(b),"meth_call_1_split":(a,b)=>a.split(b),"meth_call_2_replace":(a,b,c)=>a.replace(b,c),"meth_call_2_slice":(a,b,c)=>a.slice(b,c),"new_1":(a,b)=>new
a(b),"new_2":(a,b,c)=>new
a(b,c),"obj_3":(a,b,c,d,e,f,g,h,i,j,k,l)=>({href:a,protocol:b,host:c,hostname:d,port:e,pathname:f,search:g,hash:h,origin:i,reload:j,replace:k,assign:l}),"set_hash":(a,b)=>a.hash=b,"set_href":(a,b)=>a.href=b,"set_lastIndex":(a,b)=>a.lastIndex=b},"Js_of_ocaml__ResizeObserver.fragments":{"get_ResizeObserver":a=>a.ResizeObserver,"meth_call_1_observe":(a,b)=>a.observe(b),"meth_call_2_observe":(a,b,c)=>a.observe(b,c),"new_1":(a,b)=>new
a(b),"obj_4":()=>({}),"obj_5":()=>({}),"set_box":(a,b)=>a.box=b},"Js_of_ocaml__PerformanceObserver.fragments":{"get_PerformanceObserver":a=>a.PerformanceObserver,"meth_call_1_observe":(a,b)=>a.observe(b),"new_1":(a,b)=>new
a(b),"obj_6":()=>({}),"set_entryTypes":(a,b)=>a.entryTypes=b},"Js_of_ocaml__MutationObserver.fragments":{"get_MutationObserver":a=>a.MutationObserver,"meth_call_2_observe":(a,b,c)=>a.observe(b,c),"new_1":(a,b)=>new
a(b),"obj_7":()=>({}),"obj_8":()=>({}),"set_attributeFilter":(a,b)=>a.attributeFilter=b,"set_attributeOldValue":(a,b)=>a.attributeOldValue=b,"set_attributes":(a,b)=>a.attributes=b,"set_characterData":(a,b)=>a.characterData=b,"set_characterDataOldValue":(a,b)=>a.characterDataOldValue=b,"set_childList":(a,b)=>a.childList=b,"set_subtree":(a,b)=>a.subtree=b},"Js_of_ocaml__Jstable.fragments":{"get_Object":a=>a.Object,"get_length":a=>a.length,"meth_call_1_concat":(a,b)=>a.concat(b),"meth_call_1_keys":(a,b)=>a.keys(b),"meth_call_2_substring":(a,b,c)=>a.substring(b,c),"new_0":a=>new
a()},"Js_of_ocaml__Json.fragments":{"get_JSON":a=>a.JSON,"get_constructor":a=>a.constructor,"get_hi":a=>a.hi,"get_length":a=>a.length,"get_lo":a=>a.lo,"get_mi":a=>a.mi,"meth_call_1_stringify":(a,b)=>a.stringify(b),"meth_call_2_parse":(a,b,c)=>a.parse(b,c),"meth_call_2_stringify":(a,b,c)=>a.stringify(b,c)},"Js_of_ocaml__CSS.fragments":{"meth_call_1_test":(a,b)=>a.test(b),"new_1":(a,b)=>new
a(b)},"Js_of_ocaml__Dom_svg.fragments":{"get_SVGElement":a=>a.SVGElement,"get_document":a=>a.document,"get_tagName":a=>a.tagName,"meth_call_0_toLowerCase":a=>a.toLowerCase(),"meth_call_1_getElementById":(a,b)=>a.getElementById(b),"meth_call_2_createElementNS":(a,b,c)=>a.createElementNS(b,c)},"Js_of_ocaml__EventSource.fragments":{"get_EventSource":a=>a.EventSource,"obj_9":()=>({}),"set_withCredentials":(a,b)=>a.withCredentials=b},"Js_of_ocaml__Geolocation.fragments":{"get_geolocation":a=>a.geolocation,"get_navigator":a=>a.navigator,"obj_10":()=>({})},"Js_of_ocaml__IntersectionObserver.fragments":{"get_IntersectionObserver":a=>a.IntersectionObserver,"obj_11":()=>({})},"Js_of_ocaml__Intl.fragments":{"get_Collator":a=>a.Collator,"get_DateTimeFormat":a=>a.DateTimeFormat,"get_Intl":a=>a.Intl,"get_NumberFormat":a=>a.NumberFormat,"get_PluralRules":a=>a.PluralRules,"obj_12":a=>({localeMatcher:a}),"obj_13":(a,b,c,d,e,f)=>({localeMatcher:a,usage:b,sensitivity:c,ignorePunctuation:d,numeric:e,caseFirst:f}),"obj_14":(a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q,r,s,t)=>({dateStyle:a,timeStyle:b,calendar:c,dayPeriod:d,numberingSystem:e,localeMatcher:f,timeZone:g,hour12:h,hourCycle:i,formatMatcher:j,weekday:k,era:l,year:m,month:n,day:o,hour:p,minute:q,second:r,fractionalSecondDigits:s,timeZoneName:t}),"obj_15":(a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q,r,s,t,u)=>({compactDisplay:a,currency:b,currencyDisplay:c,currencySign:d,localeMatcher:e,notation:f,numberingSystem:g,signDisplay:h,style:i,unit:j,unitDisplay:k,useGrouping:l,roundingMode:m,roundingPriority:n,roundingIncrement:o,trailingZeroDisplay:p,minimumIntegerDigits:q,minimumFractionDigits:r,maximumFractionDigits:s,minimumSignificantDigits:t,maximumSignificantDigits:u}),"obj_16":(a,b)=>({localeMatcher:a,type:b})},"Dune__exe__Cel_wasm.fragments":{"fun_call_0":a=>a(),"fun_call_1":(a,b)=>a(b),"get_length":a=>a.length,"js_expr_2e31ecb":()=>Array.isArray,"js_expr_31221b52":()=>Math.random,"js_expr_3997d951":()=>Date.now,"js_expr_58ef6fe":()=>console.log,"js_expr_8e0b145":()=>console.error,"meth_call_1_push":(a,b)=>a.push(b),"new_0":a=>new
a(),"new_1":(a,b)=>new
a(b),"obj_0":(a,b,c,d,e,f)=>({evaluate:a,interpolate:b,evaluateGrid:c,generateAssetId:d,version:e,supportedFunctions:f}),"obj_1":()=>({}),"obj_2":(a,b,c)=>({success:a,output:b,error:c}),"obj_3":(a,b,c)=>({success:a,output:b,error:c}),"obj_4":(a,b,c)=>({success:a,result:b,error:c}),"obj_5":(a,b,c)=>({success:a,result:b,error:c}),"obj_6":(a,b,c)=>({success:a,result:b,error:c}),"obj_7":(a,b,c)=>({success:a,result:b,error:c}),"obj_8":(a,b,c)=>({success:a,result:b,error:c}),"obj_9":(a,b,c)=>({success:a,result:b,error:c})}}})(globalThis),"src":"cel_wasm.bc.wasm.assets"});
