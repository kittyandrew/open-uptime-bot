# Notification messages
notification-device-connected = Device connected!
notification-power-on = Power is back!
notification-power-off = Power outage!
notification-maintenance = Under maintenance!

# Duration parts (used to assemble duration strings)
duration-days = { $count ->
    [one] {$count} day
   *[other] {$count} days
}
duration-hours = {$count} hr
duration-minutes = {$count} min

# Duration messages
duration-power-was-off = Power was off for { $duration }
duration-power-was-on = Power was on for { $duration }
