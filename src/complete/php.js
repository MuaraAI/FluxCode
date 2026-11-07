// PHP + Laravel: language keywords, the common standard functions, Laravel
// facades, helpers, Eloquent / query-builder / request methods, Blade
// directives (in .blade.php) and Composer packages.

import { snippetCompletion as snip } from "@codemirror/autocomplete";
import { workspace } from "./workspace.js";

const words = (s) => s.trim().split(/\s+/);

const KEYWORDS = words(`abstract and array as break callable case catch class clone const continue declare default do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile enum
  extends final finally fn for foreach function global goto if implements include include_once instanceof insteadof interface isset list match namespace new or print private protected public readonly
  require require_once return static switch throw trait try unset use var while xor yield true false null self parent static string int float bool void mixed never object iterable`);

const FUNCTIONS = words(`strlen str_replace str_ireplace substr strpos stripos strrpos explode implode join trim ltrim rtrim strtolower strtoupper ucfirst ucwords lcfirst sprintf printf vsprintf number_format
  str_contains str_starts_with str_ends_with str_pad str_repeat str_split strrev str_word_count similar_text levenshtein wordwrap nl2br htmlspecialchars htmlentities html_entity_decode strip_tags addslashes stripslashes
  mb_strlen mb_substr mb_strtolower mb_strtoupper mb_str_split iconv ctype_digit ctype_alpha ctype_alnum
  array_map array_filter array_reduce array_keys array_values array_merge array_merge_recursive array_push array_pop array_shift array_unshift array_slice array_splice array_search in_array array_key_exists
  array_unique array_reverse array_sum array_product array_column array_combine array_flip array_chunk array_fill array_fill_keys array_pad array_diff array_diff_key array_intersect array_intersect_key array_walk
  array_key_first array_key_last array_is_list array_rand array_count_values range compact extract count sizeof sort rsort usort uasort uksort ksort krsort asort arsort shuffle min max array_sum
  json_encode json_decode json_last_error json_last_error_msg serialize unserialize var_dump var_export print_r die exit
  is_array is_string is_int is_integer is_float is_numeric is_null is_bool is_object is_callable is_iterable intval floatval strval boolval settype gettype get_debug_type
  round floor ceil abs pow sqrt fmod intdiv rand mt_rand random_int random_bytes pi M_PI
  date time strtotime mktime checkdate date_default_timezone_set date_create date_diff microtime hrtime sleep usleep
  file_get_contents file_put_contents fopen fclose fwrite fread fgets feof file file_exists is_file is_dir is_readable is_writable mkdir rmdir unlink rename copy filesize filemtime scandir glob basename dirname pathinfo realpath tempnam sys_get_temp_dir
  header headers_sent http_response_code setcookie session_start session_destroy session_regenerate_id session_id ob_start ob_get_clean ob_end_flush
  md5 sha1 hash hash_hmac crc32 password_hash password_verify password_needs_rehash base64_encode base64_decode bin2hex hex2bin urlencode urldecode rawurlencode http_build_query parse_url parse_str filter_var filter_input
  preg_match preg_match_all preg_replace preg_replace_callback preg_split preg_quote preg_grep
  function_exists class_exists method_exists property_exists get_class get_object_vars get_class_methods call_user_func call_user_func_array func_get_args is_a is_subclass_of define defined constant
  error_log trigger_error error_reporting set_error_handler set_exception_handler ini_set ini_get phpinfo phpversion getenv putenv memory_get_usage memory_get_peak_usage uniqid spl_autoload_register iterator_to_array array_walk_recursive
  mail curl_init curl_setopt curl_exec curl_close curl_getinfo curl_error mysqli_connect mysqli_query mysqli_fetch_assoc mysqli_real_escape_string mysqli_close`);

const SUPERGLOBALS = words(`$_GET $_POST $_REQUEST $_SERVER $_SESSION $_COOKIE $_FILES $_ENV $GLOBALS $this`);

const FACADES = {
  Route: "get post put patch delete any match resource apiResource group middleware prefix name view redirect fallback controller where domain",
  DB: "table select insert update delete statement transaction beginTransaction commit rollBack raw connection listen",
  Auth: "user id check guest attempt login logout logoutOtherDevices guard once viaRemember loginUsingId",
  Cache: "get put remember rememberForever forget has pull add increment decrement flush store tags",
  Config: "get set has all",
  Log: "info error warning debug notice critical alert emergency channel",
  Schema: "create table drop dropIfExists rename hasTable hasColumn connection",
  Storage: "disk put get exists delete url download putFile putFileAs files allFiles directories makeDirectory deleteDirectory copy move size lastModified",
  Validator: "make validate extend",
  Hash: "make check needsRehash",
  Mail: "to cc bcc send queue later raw",
  Queue: "push later bulk",
  Event: "dispatch listen subscribe fake assertDispatched",
  Gate: "define allows denies check authorize before after policy",
  Session: "get put has forget flush flash regenerate all pull",
  Http: "get post put patch delete withHeaders withToken withBasicAuth timeout retry asForm asJson attach fake pool",
  Str: "slug random uuid camel snake kebab studly title upper lower limit words contains startsWith endsWith replace after before between plural singular padLeft padRight",
  Arr: "get set has only except pluck flatten first last where wrap collapse dot except forget random shuffle sort",
  Redirect: "to route back away refresh with withErrors",
  Response: "json download file stream view make",
  View: "make share composer exists",
  Artisan: "call queue command output",
  Bus: "dispatch chain batch",
  Notification: "send route fake",
  Password: "sendResetLink reset broker",
  Crypt: "encrypt decrypt encryptString decryptString",
  Cookie: "make queue get has forget",
  URL: "to route action signedRoute temporarySignedRoute current previous full",
  App: "environment isLocal isProduction make bind singleton setLocale getLocale version",
  Blade: "directive component if",
  Request: "all input only except has filled query route user header",
  File: "get put exists delete copy move size extension name basename directories files makeDirectory deleteDirectory",
  RateLimiter: "for attempt tooManyAttempts clear",
  Process: "run start pipe timeout path env",
  Concurrency: "run defer",
  Vite: "asset useBuildDirectory",
};

const HELPERS = words(`route view redirect request response config env auth app abort abort_if abort_unless asset url old session trans __ collect now today dd dump logger info bcrypt
  back csrf_field csrf_token method_field mix vite cache cookie data_get data_set event dispatch report rescue retry tap throw_if throw_unless value with optional
  str class_basename blank filled head last once resolve policy fake broadcast validator storage_path base_path app_path config_path database_path public_path resource_path lang_path
  Str::of to_route response()->json`);

const ELOQUENT = words(`where orWhere whereIn whereNotIn whereNull whereNotNull whereBetween whereDate whereHas whereDoesntHave whereColumn whereRaw orderBy orderByDesc latest oldest limit take skip offset
  get first firstOrFail find findOrFail findMany create update updateOrCreate firstOrCreate firstOrNew insert upsert delete destroy forceDelete restore withTrashed onlyTrashed save saveQuietly fresh refresh
  pluck count sum avg max min exists doesntExist paginate simplePaginate cursorPaginate chunk chunkById each lazy toArray toJson load loadMissing with withCount withSum without
  hasMany hasOne belongsTo belongsToMany morphTo morphMany morphOne morphToMany hasManyThrough hasOneThrough attach detach sync syncWithoutDetaching toggle associate dissociate
  increment decrement select selectRaw addSelect distinct join leftJoin rightJoin crossJoin groupBy groupByRaw having havingRaw union lockForUpdate sharedLock
  scope fillable guarded casts hidden appends touches timestamps table primaryKey keyType incrementing factory newQuery query when unless tap dd dump toSql
  input all only except has filled missing query post json file hasFile validate validated safe route user header bearerToken ip method is ajax expectsJson wantsJson boolean integer string date enum
  map filter each reduce sortBy sortByDesc groupBy keyBy values keys unique flatten pluck sum avg implode isEmpty isNotEmpty contains search merge push put pull forget random reverse slice splice chunk zip`);

const USES = words(`Illuminate\\Http\\Request Illuminate\\Http\\Response Illuminate\\Http\\JsonResponse Illuminate\\Http\\RedirectResponse Illuminate\\Support\\Facades\\Route Illuminate\\Support\\Facades\\DB
  Illuminate\\Support\\Facades\\Auth Illuminate\\Support\\Facades\\Cache Illuminate\\Support\\Facades\\Log Illuminate\\Support\\Facades\\Schema Illuminate\\Support\\Facades\\Storage Illuminate\\Support\\Facades\\Hash
  Illuminate\\Support\\Facades\\Mail Illuminate\\Support\\Facades\\Validator Illuminate\\Support\\Facades\\Http Illuminate\\Support\\Facades\\Gate Illuminate\\Support\\Facades\\Event Illuminate\\Support\\Facades\\Queue
  Illuminate\\Support\\Str Illuminate\\Support\\Arr Illuminate\\Support\\Collection Illuminate\\Support\\Carbon Illuminate\\Support\\ServiceProvider Illuminate\\Support\\Facades\\Config
  Illuminate\\Database\\Eloquent\\Model Illuminate\\Database\\Eloquent\\Factories\\HasFactory Illuminate\\Database\\Eloquent\\SoftDeletes Illuminate\\Database\\Eloquent\\Relations\\HasMany Illuminate\\Database\\Eloquent\\Relations\\BelongsTo
  Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany Illuminate\\Database\\Eloquent\\Builder Illuminate\\Database\\Migrations\\Migration Illuminate\\Database\\Schema\\Blueprint Illuminate\\Database\\Seeder
  Illuminate\\Foundation\\Auth\\User Illuminate\\Foundation\\Http\\FormRequest Illuminate\\Contracts\\Auth\\MustVerifyEmail Illuminate\\Notifications\\Notifiable Illuminate\\Auth\\Access\\HandlesAuthorization
  Illuminate\\Validation\\Rule Illuminate\\Validation\\ValidationException Illuminate\\Console\\Command Illuminate\\Queue\\InteractsWithQueue Illuminate\\Contracts\\Queue\\ShouldQueue Illuminate\\Bus\\Queueable
  Illuminate\\Mail\\Mailable Illuminate\\Notifications\\Notification Illuminate\\Support\\Facades\\Artisan Illuminate\\View\\Component Illuminate\\Http\\Resources\\Json\\JsonResource
  App\\Models\\User App\\Http\\Controllers\\Controller App\\Http\\Requests App\\Models App\\Services App\\Http\\Middleware Carbon\\Carbon Symfony\\Component\\HttpFoundation\\Response`);

export const COMPOSER_POPULAR = words(`laravel/framework laravel/sanctum laravel/breeze laravel/jetstream laravel/horizon laravel/telescope laravel/tinker laravel/pint laravel/sail laravel/socialite laravel/cashier
  laravel/scout laravel/passport laravel/fortify laravel/octane laravel/pulse laravel/reverb livewire/livewire inertiajs/inertia-laravel filament/filament spatie/laravel-permission spatie/laravel-medialibrary
  spatie/laravel-backup spatie/laravel-activitylog spatie/laravel-query-builder barryvdh/laravel-debugbar barryvdh/laravel-dompdf barryvdh/laravel-ide-helper maatwebsite/excel intervention/image
  guzzlehttp/guzzle symfony/console symfony/http-foundation monolog/monolog phpunit/phpunit pestphp/pest fakerphp/faker nesbot/carbon ramsey/uuid vlucas/phpdotenv league/flysystem league/csv
  predis/predis doctrine/dbal nunomaduro/collision mockery/mockery tymon/jwt-auth yajra/laravel-datatables-oracle stripe/stripe-php twilio/sdk aws/aws-sdk-php phpmailer/phpmailer slim/slim
  illuminate/support dompdf/dompdf mpdf/mpdf tcpdf/tcpdf firebase/php-jwt nikic/fast-route league/oauth2-client midtrans/midtrans-php xendit/xendit-php`);

const BLADE = words(`@if @elseif @else @endif @unless @endunless @isset @endisset @empty @endempty @foreach @endforeach @forelse @endforelse @for @endfor @while @endwhile @break @continue
  @extends @section @endsection @yield @show @parent @include @includeIf @includeWhen @includeUnless @includeFirst @each @component @endcomponent @slot @endslot @props @aware
  @csrf @method @auth @endauth @guest @endguest @can @elsecan @endcan @cannot @endcannot @canany @endcanany @error @enderror @json @js @php @endphp @push @endpush @pushOnce @prepend @endprepend @stack
  @once @endonce @class @style @checked @selected @disabled @readonly @required @vite @livewire @livewireStyles @livewireScripts @env @endenv @production @endproduction @lang @inject @verbatim @endverbatim
  @dd @dump @switch @case @default @endswitch @hasSection @sectionMissing @session @endsession @fragment @endfragment @use @teleport @endteleport`);

const facadeOptions = Object.keys(FACADES).map((f) => ({ label: f, type: "class", detail: "Laravel facade", boost: 5 }));
const statics = [
  ...KEYWORDS.map((k) => ({ label: k, type: "keyword" })),
  ...FUNCTIONS.map((f) => ({ label: f, type: "function", detail: "PHP" })),
  ...SUPERGLOBALS.map((v) => ({ label: v, type: "variable", detail: "superglobal" })),
  ...facadeOptions,
  ...HELPERS.filter((h) => /^[a-z_]+$/.test(h)).map((h) => ({ label: h, type: "function", detail: "Laravel helper", boost: 3 })),
  snip("public function ${name}(${params})\n{\n\t${}\n}", { label: "pubf", detail: "public method", type: "snippet" }),
  snip("foreach (${$items} as ${$item}) {\n\t${}\n}", { label: "foreach", detail: "foreach loop", type: "snippet" }),
  snip("Route::${get}('${/path}', [${Controller}::class, '${index}']);", { label: "route", detail: "Laravel route", type: "snippet" }),
  snip("$request->validate([\n\t'${field}' => '${required}',\n]);", { label: "validate", detail: "request validation", type: "snippet" }),
  snip("return view('${name}', [\n\t${}\n]);", { label: "view", detail: "return view", type: "snippet" }),
  snip("return redirect()->route('${name}')->with('${status}', '${Saved}');", { label: "redirect", detail: "redirect to route", type: "snippet" }),
  snip("return response()->json([\n\t${}\n]);", { label: "json", detail: "JSON response", type: "snippet" }),
  snip("Schema::create('${table}', function (Blueprint $table) {\n\t$table->id();\n\t${}\n\t$table->timestamps();\n});", { label: "schema", detail: "migration table", type: "snippet" }),
  snip("public function ${name}()\n{\n\treturn $this->${hasMany}(${Model}::class);\n}", { label: "relation", detail: "Eloquent relation", type: "snippet" }),
  snip("<?php\n\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Http\\Request;\n\nclass ${Name}Controller extends Controller\n{\n\tpublic function index()\n\t{\n\t\t${}\n\t}\n}", { label: "controller", detail: "Laravel controller", type: "snippet" }),
  snip("<?php\n\nnamespace App\\Models;\n\nuse Illuminate\\Database\\Eloquent\\Factories\\HasFactory;\nuse Illuminate\\Database\\Eloquent\\Model;\n\nclass ${Name} extends Model\n{\n\tuse HasFactory;\n\n\tprotected $fillable = [${}];\n}", { label: "model", detail: "Eloquent model", type: "snippet" }),
  snip("<?php\n\n${}", { label: "php", detail: "<?php", type: "snippet" }),
];
const members = ELOQUENT.map((m) => ({ label: m, type: "method", detail: "Laravel" }));
const useOptions = USES.map((u) => ({ label: u, type: "namespace", detail: "use" }));
const bladeOptions = BLADE.map((b) => ({ label: b, type: "keyword", detail: "Blade" }));

export function phpCompletion(blade) {
  return [
    (context) => {
      const u = context.matchBefore(/^\s*use\s+[\w\\]*$/);
      if (u) { const w = context.matchBefore(/[\w\\]*$/); return { from: w.from, options: useOptions, validFor: /^[\w\\]*$/ }; }
      const facade = context.matchBefore(/\b([A-Z][A-Za-z]*)::\w*$/);
      if (facade) {
        const name = /([A-Z][A-Za-z]*)::/.exec(facade.text)[1];
        const list = FACADES[name];
        const w = context.matchBefore(/\w*$/);
        if (list) return { from: w.from, options: words(list).map((m) => ({ label: m, type: "method", detail: name })), validFor: /^\w*$/ };
      }
      if (context.matchBefore(/->\w*$/)) { const w = context.matchBefore(/\w*$/); return { from: w.from, options: members, validFor: /^\w*$/ }; }
      if (blade) {
        const at = context.matchBefore(/@\w*$/);
        if (at) return { from: at.from, options: bladeOptions, validFor: /^@\w*$/ };
      }
      const word = context.matchBefore(/\$?\w*$/);
      if (!word || (!word.text && !context.explicit)) return null;
      return { from: word.from, options: statics, validFor: /^\$?\w*$/ };
    },
  ];
}
