using System.Text.Json;
using System.Text.Json.Serialization;
using DemoFile;
using DemoFile.Game.Cs;

if (args.Length is < 4 or > 5)
{
    Console.Error.WriteLine("usage: ApproximateLayerAdapter <demo.dem> <profile.json> <per-tick.json> <downsampled.json> [--generic-fallback]");
    return 2;
}

var demoPath = args[0];
var jsonOptions = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };
var profile = JsonSerializer.Deserialize<Profile>(File.ReadAllText(args[1]), jsonOptions)
    ?? throw new InvalidDataException("profile parse failed");
if (profile.Capsules.Count != 19 || profile.ExactModelCalibration || profile.Confidence != "generic_fallback")
    throw new InvalidDataException("profile must be exactly 19 generic non-calibrated capsules");

var fallback = args.Length == 5 && args[4] == "--generic-fallback";
var perTick = new JsonArrayWriter(args[2], "per_tick", profile.ProfileId, profile.Confidence, jsonOptions);
var downsampled = new JsonArrayWriter(args[3], "downsampled_10hz", profile.ProfileId, profile.Confidence, jsonOptions);
var observedDefinitionPlayers = new HashSet<uint>();
var recordCount = 0;
var downsampledCount = 0;

var demo = new CsDemoParser();
void EmitRecords(int tick)
{
    foreach (var controller in demo.Entities.OfType<CCSPlayerController>())
    {
        var pawn = controller.PlayerPawn;
        if (pawn is null || !pawn.IsAlive)
            continue;

        var definition = controller.PawnCharacterDefIndex;
        if (definition != 5037 && !fallback)
            continue;

        var duck = Math.Clamp((pawn.MovementServices as CCSPlayer_MovementServices)?.DuckAmount ?? 0f, 0f, 1f);
        var record = SpatialRecord.Create(
            tick,
            pawn.EntityIndex.Value,
            definition,
            pawn.Origin,
            pawn.EyeAngles.Yaw,
            duck,
            profile,
            definition == 5037 ? "definition_5037_to_ctm_sas_profile" : "configured_generic_fallback"
        );
        perTick.Write(record);
        recordCount++;
        if (definition == 5037)
            observedDefinitionPlayers.Add(pawn.EntityIndex.Value);
        if (tick % 6 == 0)
        {
            downsampled.Write(record);
            downsampledCount++;
        }
    }
}

var reader = DemoFileReader.Create(demo, File.OpenRead(demoPath));
await reader.StartReadingAsync(default);
var lastTick = int.MinValue;
while (await reader.MoveNextAsync(default))
{
    if (demo.CurrentDemoTick.Value != lastTick)
    {
        lastTick = demo.CurrentDemoTick.Value;
        EmitRecords(lastTick);
    }
}
perTick.Close();
downsampled.Close();

if (recordCount == 0 && !fallback)
    Console.Error.WriteLine("No live player-pawn records with PawnCharacterDefIndex=5037 were emitted; rerun with --generic-fallback only for explicitly configured generic visualization.");
Console.WriteLine(JsonSerializer.Serialize(new {
    players_with_def_index_5037 = observedDefinitionPlayers.Count,
    approximate_spatial_records_per_tick = recordCount,
    approximate_spatial_records_downsampled = downsampledCount,
    evidence_allowed = false,
    usage_scope = "exploratory_functional",
    confidence = "generic_fallback",
    m_hModel_binding = "not_used"
}));
return 0;

public sealed record Profile(string ProfileId, bool ExactModelCalibration, CrouchTransform CrouchTransform, string Confidence, List<Capsule> Capsules);
public sealed record CrouchTransform(float UpperBodyPivotZ, float UpperBodyDropPerDuck, float UpperBodyZScaleAtFullDuck, float LegZScaleAtFullDuck);
public sealed record Capsule(string Id, string Group, float[] A, float[] B, float Radius);

public sealed record Vec(float X, float Y, float Z);
public sealed record CapsuleWorld(string Id, Vec CenterWorld, float Radius, float Height, float OrientationYawDeg);
public sealed record CrouchScale(float SpineHead, float Legs);
public sealed record Provenance(
    bool EvidenceAllowed,
    string UsageScope,
    string Derivation,
    [property: JsonPropertyName("m_hModel_binding")] string MModelBinding,
    bool ExactGeometryClaimed,
    bool ExactSkeletonClaimed,
    bool ExactHitboxesClaimed,
    bool Ag2BoneMatricesClaimed
);
public sealed record SpatialRecord(
    int Tick,
    uint EntityIndex,
    ushort PawnCharacterDefIndex,
    string ProfileTag,
    string Confidence,
    Vec Origin,
    float YawDeg,
    float DuckAmount,
    CrouchScale CrouchScale,
    List<CapsuleWorld> Capsules,
    Provenance Provenance
)
{
    public static SpatialRecord Create(int tick, uint entityIndex, ushort definition, Vector origin, float yaw, float duck, Profile profile, string derivation)
    {
        var spine = 1f + (profile.CrouchTransform.UpperBodyZScaleAtFullDuck - 1f) * duck;
        var legs = 1f + (profile.CrouchTransform.LegZScaleAtFullDuck - 1f) * duck;
        var capsules = profile.Capsules.Select(c => Transform(c, origin, yaw, duck, profile.CrouchTransform)).ToList();
        if (capsules.Count != 19)
            throw new InvalidDataException("approximate record must have 19 capsules");
        return new SpatialRecord(
            tick, entityIndex, definition, "ctm_sas_generic_19_capsules", "generic_fallback",
            new Vec(origin.X, origin.Y, origin.Z), yaw, duck, new CrouchScale(spine, legs), capsules,
            new Provenance(false, "exploratory_functional", derivation, "not_used", false, false, false, false)
        );
    }

    private static CapsuleWorld Transform(Capsule capsule, Vector origin, float yaw, float duck, CrouchTransform crouch)
    {
        var a = TransformPoint(capsule.A, capsule.Group, origin, yaw, duck, crouch);
        var b = TransformPoint(capsule.B, capsule.Group, origin, yaw, duck, crouch);
        var center = new Vec((a.X + b.X) / 2f, (a.Y + b.Y) / 2f, (a.Z + b.Z) / 2f);
        var height = MathF.Sqrt(MathF.Pow(b.X - a.X, 2) + MathF.Pow(b.Y - a.Y, 2) + MathF.Pow(b.Z - a.Z, 2));
        return new CapsuleWorld(capsule.Id, center, capsule.Radius, height, yaw);
    }

    private static Vec TransformPoint(float[] local, string group, Vector origin, float yaw, float duck, CrouchTransform crouch)
    {
        var z = local[2];
        if (group is "head" or "neck" or "torso" or "arm" or "pelvis")
        {
            var scale = 1f + (crouch.UpperBodyZScaleAtFullDuck - 1f) * duck;
            z = crouch.UpperBodyPivotZ + (z - crouch.UpperBodyPivotZ) * scale - crouch.UpperBodyDropPerDuck * duck;
        }
        else if (group == "leg")
        {
            z *= 1f + (crouch.LegZScaleAtFullDuck - 1f) * duck;
        }
        var r = yaw * MathF.PI / 180f;
        var x = MathF.Cos(r) * local[0] - MathF.Sin(r) * local[1];
        var y = MathF.Sin(r) * local[0] + MathF.Cos(r) * local[1];
        return new Vec(origin.X + x, origin.Y + y, origin.Z + z);
    }
}

public sealed class JsonArrayWriter : IDisposable
{
    private readonly FileStream stream;
    private readonly Utf8JsonWriter json;
    private readonly JsonSerializerOptions options;
    public JsonArrayWriter(string path, string resolution, string profileId, string confidence, JsonSerializerOptions options)
    {
        this.options = options;
        stream = File.Create(path);
        json = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = false });
        json.WriteStartObject();
        json.WriteNumber("schema_version", 1);
        json.WriteString("demo_sha256", "7c5bad6f12be4cb7be81a996afa8adbda4a8d3182a0e77c26c7f8a47601bd917");
        json.WriteString("map", "de_ancient");
        json.WriteString("resolution", resolution);
        json.WriteString("profile_id", profileId);
        json.WriteString("confidence", confidence);
        json.WriteStartArray("records");
    }
    public void Write(SpatialRecord record) => JsonSerializer.Serialize(json, record, options);
    public void Close()
    {
        json.WriteEndArray();
        json.WriteEndObject();
        json.Flush();
        Dispose();
    }
    public void Dispose()
    {
        json.Dispose();
        stream.Dispose();
    }
}
