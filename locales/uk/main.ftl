# Notification messages
notification-device-connected = Девайс під'єднано!
notification-power-on = Світло з'явилося!
notification-power-off = Відключення світла!
notification-maintenance = На обслуговуванні!

# Duration parts (used to assemble duration strings)
duration-days = { $count ->
    [one] {$count} день
    [few] {$count} дні
   *[other] {$count} днів
}
duration-hours = {$count} год
duration-minutes = {$count} хв

# Duration messages
duration-power-was-off = Світла не було { $duration }
duration-power-was-on = Світло було { $duration }
