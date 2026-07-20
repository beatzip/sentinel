import json

with open(r'C:\Users\User\Desktop\123\demka_decoded.json') as f:
    data = json.load(f)

players = [p for p in data['players'] if p.get('evidence_count', 0) > 0]
players.sort(key=lambda x: x['scores']['overall'], reverse=True)

print('=== TOP-5 PLAYERS BY ROTATION JUSTIFICATION EVIDENCE ===')
print()

for i, p in enumerate(players[:5]):
    print(f"{i+1}. {p['name']} ({p['team']})")
    print(f"   Overall: {p['scores']['overall']:.3f}")
    print(f"   Evidence: {p['evidence_count']} items")
    print(f"   Categories:")
    for cat, val in p['scores']['categories'].items():
        print(f"     {cat}: {val:.3f}")
    
    # Show first few evidence items
    if p['evidence']:
        print(f"   Sample evidence:")
        for ev in p['evidence'][:3]:
            print(f"     - {ev['feature']}: score={ev['score']:.3f} - {ev['reason'][:80]}")
    print()
